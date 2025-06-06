import math
import types
import typing
import importlib
import sys
_torch_fast_mask_enabled = sys.maxsize.bit_length() == 63
from .kbnf import InternalEngine, AcceptTokenResult, Vocabulary,Config
_slice_converters = []
_fast_mask_logits = []

def _try_register_slice_converter(module_name:str,
                        obtain_converter:typing.Callable[[types.ModuleType],
                                                        typing.Callable[[typing.Any],
                                                                        typing.Optional[typing.Tuple[typing.Any,int,int]]]]):
    try:
        module = importlib.import_module(module_name)
        _slice_converters.append(obtain_converter(module))
    except ImportError:
        pass

def _try_register_fast_mask_logits(module_name:str,
                        fast_mask_logits:typing.Callable[[types.ModuleType],
                                                        typing.Callable[[typing.Any, "Engine"],
                                                                        typing.Optional[typing.Any]]]):
    try:
        module = importlib.import_module(module_name)
        _fast_mask_logits.append(fast_mask_logits(module))
    except ImportError:
        pass

def _torch_slice_converter(module:types.ModuleType):
    def convert_slice(tensor:typing.Any)->typing.Optional[typing.Tuple[typing.Any,int,int]]:
        if isinstance(tensor, module.Tensor):
            assert tensor.dim() == 1 or tensor.dim() == 2 and tensor.shape[0] == 1,\
            f"Only tensors with shape (n) or (1,n) are supported, while the actual tensor shape is {tensor.shape}"
            tensor = tensor.to(device="cpu",dtype=module.float32,memory_format=module.contiguous_format)
            ptr = tensor.data_ptr()
            assert ptr % 4 == 0, f"The tensor data pointer which points to {ptr} is not aligned to 4 bytes"
            return tensor, ptr, tensor.shape[-1]
        return None
    return convert_slice

def _torch_fast_mask_logits(module:types.ModuleType):
    ninf = -math.inf
    def mask_logits_fast(tensor:typing.Any, engine:"Engine")->typing.Optional[typing.Any]:
        if isinstance(tensor, module.Tensor):
            assert tensor.dim() == 1, f"Only 1D tensor is supported, while the actual tensor shape is {tensor.shape}"
            index = engine.get_index_of_allowed_token_ids()
            num_of_disallowed = engine.get_number_of_disallowed_token_ids()
            if index not in engine._cache:
                length = num_of_disallowed
                if length == 0: # Rust FFI requires non-null pointer
                    return tensor
                pinned = tensor.is_cuda # only pin if the logits is on CUDA, which implies the user is using CUDA for its LLM
                disallowed = module.empty((length,), device="cpu",dtype=module.int64, pin_memory=pinned)
                data_ptr = disallowed.data_ptr()
                assert data_ptr != 0, f"The indices data pointer which points to {data_ptr} is null"
                assert data_ptr % 8 == 0, f"The indices data pointer which points to {data_ptr} is not aligned to 8 bytes"
                engine.write_disallowed_token_ids_to_buffer(data_ptr, length)
                length = engine.get_number_of_allowed_token_ids()
                allowed = module.empty((length,), device="cpu",dtype=module.int64, pin_memory=pinned)
                data_ptr = allowed.data_ptr()
                assert data_ptr != 0, f"The allowed data pointer which points to {data_ptr} is null"
                assert data_ptr % 8 == 0, f"The allowed data pointer which points to {data_ptr} is not aligned to 8 bytes"
                engine.write_allowed_token_ids_to_buffer(data_ptr, length)
                engine._cache[index] = (disallowed, allowed)
            else:
                disallowed, allowed = engine._cache[index]
            if num_of_disallowed>tensor.shape[-1]/2: # we have more disallowed than allowed
                new_tensor = module.full_like(tensor,fill_value=ninf)
                allowed = allowed.to(device=tensor.device,non_blocking=True)
                new_tensor.put_(allowed, tensor.take(allowed))
                return new_tensor
            else: # we have more allowed than disallowed
                tensor.index_fill_(0,disallowed.to(device=tensor.device,non_blocking=True),ninf)
                return tensor
        return None
    return mask_logits_fast

def _numpy_slice_converter(module:types.ModuleType):
    def convert_slice(array:typing.Any)->typing.Optional[typing.Tuple[typing.Any,int,int]]:
        if isinstance(array, module.ndarray):
            assert array.ndim == 1 or array.ndim == 2 and array.shape[0] == 1,\
            f"Only array with shape (n) or (1,n) are supported, while the actual array shape is {array.shape}"
            if (array.dtype != module.float32 or not array.flags["CARRAY"]):
                array = array.astype(module.float32, order="C")
            ptr = array.ctypes.data
            assert ptr % 4 == 0, f"The tensor data pointer which points to {ptr} is not aligned to 4 bytes"
            return array, ptr, array.shape[-1]
        return None
    return convert_slice

def _convert_logits_to_slice(logits:typing.Any)->typing.Tuple[typing.Any,int,int]:
    for converter in _slice_converters:
        converted = converter(logits)
        if converted is not None:
            return converted
    raise TypeError(f"Unsupported type of logits: {type(logits)}")

def _mask_logits_fast(logits:typing.Any,engine:"Engine")->typing.Optional[typing.Any]:
    for masker in _fast_mask_logits:
        masked = masker(logits, engine)
        if masked is not None:
            return masked
    return None

class Engine:
    def __init__(self, kbnf_syntax_grammar_str, vocabulary, config=None):
        self._internal = InternalEngine(kbnf_syntax_grammar_str, vocabulary, config)
        self._cache = {}

    def try_accept_new_token(self, token_id:int)->AcceptTokenResult:
        return self._internal.try_accept_new_token(token_id)
    
    def try_accept_new_bytes(self, _bytes:bytes)->AcceptTokenResult:
        return self._internal.try_accept_new_bytes(_bytes)
    
    def get_allowed_token_ids_from_last_computation(self)->typing.List[int]:
        return self._internal.get_allowed_token_ids_from_last_computation()
    
    def compute_allowed_token_ids(self)->typing.List[int]:
        return self._internal.compute_allowed_token_ids()
    
    def get_disallowed_token_ids_from_last_computation(self)->typing.List[int]:
        return self._internal.get_disallowed_token_ids_from_last_computation()
    
    def check_if_token_is_allowed(self, token_id:int)->bool:
        return self._internal.check_if_token_is_allowed(token_id)
    
    def get_number_of_disallowed_token_ids(self)->int:
        return self._internal.get_number_of_disallowed_token_ids()
    
    def get_number_of_allowed_token_ids(self)->int:
        return self._internal.get_number_of_allowed_token_ids()
    
    def get_index_of_allowed_token_ids(self)->bytes:
        return self._internal.get_index_of_allowed_token_ids()
    
    def write_disallowed_token_ids_to_buffer(self, ptr:int, length:int)->None:
        self._internal.write_disallowed_token_ids_to_buffer(ptr, length)
    
    def write_allowed_token_ids_to_buffer(self, ptr:int, length:int)->None:
        self._internal.write_allowed_token_ids_to_buffer(ptr, length)

    def fill_torch_bitmask(self, bitmask_ptr:int)->None:
        self._internal.fill_torch_bitmask(bitmask_ptr)

    def is_finished(self)->bool:
        return self._internal.is_finished()
    
    def get_vocab(self)->Vocabulary:
        return self._internal.get_vocab()

    def shrink_to_fit(self)->None:
        self._internal.shrink_to_fit()

    def reset(self)->None:
        self._internal.reset()
    
    def mask_logits(self, logits):
        """
Masks the logits based on last computed token IDs.
These token IDs can also be obtained from [`EngineLike::allowed_token_ids_from_last_computation`].

Last computation is the last [`EngineLike::compute_allowed_token_ids`] or [`EngineLike::update_logits`] called.
In other words, [`EngineLike::try_accept_new_token`] DOES NOT compute the allowed token IDs
and hence DOES NOT affect the masking!

# Arguments

* `logits`: The logits to be masked. `numpy.ndarray` is supported by default.
`torch.Tensor` is supported if PyTorch is installed. The shape of the logits should be `(n,)`.
The logits will be updated in-place if any of the following conditions is met:
    * logits type is numpy.ndarray and all of the following conditions are met:
        * The logits data type is `float32`.
        * The underlying data buffer are contiguous AND on CPU.
        * The data pointer is aligned to 4 bytes.
# Returns

The masked logits. The shape of the returned logits is the same as the input logits. 
The returned logits is the same object as the input logits if the input logits is updated in-place.
Otherwise, a new object with the same type as the input logits is returned.

# Exceptions

This method may raise the following exceptions:
    * TypeError: When the logits type is not supported.
    * AssertionError: When the logits shape is not supported or the memory allocator returns an unaligned pointer.
    * ValueError: When the logits length is too short.
    * torch.RuntimeError: When the logits type is torch.Tensor and the logits length is too short.
        """
        result = _mask_logits_fast(logits, self)
        if result is not None:
            return result
        logits, ptr, size = _convert_logits_to_slice(logits)
        self._internal.mask_logits(ptr, size)
        return logits
    
    def update_logits(self, token_id:int, logits)->typing.Tuple[typing.Any,AcceptTokenResult]:
        """
Try to accept the token ID and if succeeds, update the given logits array.

# Arguments

* `token_id`: The token ID to be accepted.
* `logits`: The logits to be updated. `numpy.ndarray` is supported by default.
`torch.Tensor` is supported if PyTorch is installed. The shape of the logits should be `(1, n)` or `(n,)`.
The logits will be updated in-place if:
    * The logits data type is `float32`.
    * The underlying data buffer are contiguous AND on CPU.
    * The data pointer is aligned to 4 bytes.

# Returns

a tuple (logits, result). The logits is the same object as the input logits if the input logits is updated in-place.
Otherwise, a new object with the same type as the input logits is returned. 
The `result` is the result of accepting the token ID.

# Exceptions

This method may raise the following exceptions:
    * TypeError: When the logits type is not supported.
    * AssertionError: When the logits shape is not supported or the memory allocator returns an unaligned pointer.
    * ValueError: When the logits length is too short.
        """
        logits, ptr, size = _convert_logits_to_slice(logits)
        result = self._internal.update_logits(token_id, ptr, size)
        return logits, result
    
    def __repr__(self)->str:
        return f"Engine({self._internal.__repr__()}, {self._cache})"
    
    def __str__(self)->str:
        return self.__repr__()

    def __copy__(self):
        new_engine = Engine.__new__(Engine)
        new_engine._internal = self._internal.__copy__()
        new_engine._cache = {}
        return new_engine
    
    def __deepcopy__(self, memo):
        new_engine = Engine.__new__(Engine)
        new_engine._internal = self._internal.__deepcopy__(memo)
        new_engine._cache = {}
        return new_engine

_try_register_slice_converter("torch", _torch_slice_converter)
_try_register_slice_converter("numpy", _numpy_slice_converter)
if _torch_fast_mask_enabled:
    _try_register_fast_mask_logits("torch", _torch_fast_mask_logits)