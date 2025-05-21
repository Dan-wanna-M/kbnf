"""
Adapted from XGrammar v0.1.10
"""
from typing import List, Optional, Union

import torch
import triton
import triton.language as tl
from .engine import Engine

@triton.jit
def _apply_token_bitmask_inplace_kernel(
    logits_ptr,
    bitmask_ptr,
    indices_ptr,
    num_rows,
    vocab_size,
    bitmask_size,
    NUM_SMS: tl.constexpr,
    BLOCK_SIZE: tl.constexpr,
):
    pid = tl.program_id(0)
    num_blocks = tl.cdiv(vocab_size, BLOCK_SIZE)
    for work_id in tl.range(pid, num_rows * num_blocks, NUM_SMS):
        block_offset = (work_id % num_blocks) * BLOCK_SIZE
        row_id = work_id // num_blocks
        batch_id = tl.load(indices_ptr + row_id)
        offsets = block_offset + tl.arange(0, BLOCK_SIZE)
        bitmask_offsets = block_offset // 32 + tl.arange(0, BLOCK_SIZE // 32)
        vocab_mask = offsets < vocab_size
        packed_bitmask_mask = bitmask_offsets < bitmask_size
        packed_bitmask = tl.load(
            bitmask_ptr + batch_id * bitmask_size + bitmask_offsets, packed_bitmask_mask
        )
        bitmask = ((packed_bitmask[:, None] >> (tl.arange(0, 32)[None, :])) & 1) == 0
        bitmask = bitmask.reshape(BLOCK_SIZE)

        tl.store(logits_ptr + batch_id * vocab_size + offsets, -float("inf"), vocab_mask & bitmask)


def _apply_token_bitmask_inplace_triton(
    logits: torch.Tensor,
    bitmask: torch.Tensor,
    indices: Optional[Union[List[int], torch.Tensor]] = None,
):
    def ceil_div(a, b):
        return (a + b - 1) // b

    NUM_SMS = torch.cuda.get_device_properties("cuda").multi_processor_count
    BLOCK_SIZE = 4096
    # Check input tensor shapes.
    if logits.ndim == 2:
        batch_size, vocab_size = logits.shape
    elif logits.ndim == 1:
        batch_size = 1
        (vocab_size,) = logits.shape
    else:
        raise ValueError(f"Invalid logits tensor shape {logits.shape}")

    if indices is None:
        indices = torch.arange(batch_size, dtype=torch.int32, device=logits.device)
    elif isinstance(indices, list):
        indices = torch.tensor(indices, dtype=torch.int32, device=logits.device)

    grid = lambda meta: (NUM_SMS,)

    _apply_token_bitmask_inplace_kernel[grid](
        logits,
        bitmask,
        indices,
        indices.shape[0],
        vocab_size,
        ceil_div(vocab_size, 32),
        NUM_SMS,
        BLOCK_SIZE,
        num_warps=BLOCK_SIZE // 32 // (16 // logits.element_size()),
        num_stages=3,
    )

def mask_logits_inplace(logits: torch.Tensor, bitmasks: list[torch.Tensor]|torch.Tensor,engines:list[Engine], indices: Optional[Union[List[int], torch.Tensor]] = None):
    """
    Masks logits in-place using token bitmasks filled by KBNF engines.
    
    This function fills token bitmasks by KBNF engines and applies them to logits tensors efficiently using Triton kernels.
    It sets disallowed token probabilities to negative infinity.
    
    Args:
        logits: Tensor of shape (batch_size, vocab_size) or (vocab_size,) containing logits to be masked
        bitmasks: Either a single bitmask tensor or a list of bitmask tensors, one per engine
        engines: List of KBNF Engine objects that will fill the bitmasks
        indices: Optional indices to select specific batch elements to apply masking to.
                 If None, masking is applied to all batch elements.
    
    Returns:
        None - the logits tensor is modified in-place
    """

    if bitmasks.ndim == 1:
        bitmasks = [bitmasks]
    for engine, bitmask in zip(engines, bitmasks):
        engine.fill_torch_bitmask(bitmask.data_ptr())
    _apply_token_bitmask_inplace_triton(logits, bitmask, indices)
