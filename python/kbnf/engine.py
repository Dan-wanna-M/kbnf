import types
import typing
import importlib
from .kbnf import InternalEngine, AcceptTokenResult, Vocabulary,Config
_slice_converters = []

def _try_register_slice_converter(module_name:str,
                        obtain_converter:typing.Callable[[types.ModuleType],
                                                        typing.Callable[[typing.Any],
                                                                        typing.Optional[typing.Tuple[typing.Any,int,int]]]]):
    try:
        module = importlib.import_module(module_name)
        _slice_converters.append(obtain_converter(module))
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

class Engine(InternalEngine):
    def mask_logits(self, logits):
        """
Masks the logits based on last computed token IDs.
These token IDs can also be obtained from [`EngineLike::allowed_token_ids_from_last_computation`].

Last computation is the last [`EngineLike::compute_allowed_token_ids`] or [`EngineLike::update_logits`] called.
In other words, [`EngineLike::try_accept_new_token`] DOES NOT compute the allowed token IDs
and hence DOES NOT affect the masking!

# Arguments

* `logits`: The logits to be masked. `numpy.ndarray` is supported by default.
`torch.Tensor` is supported if PyTorch is installed. The shape of the logits should be `(1, n)` or `(n,)`.
The logits will be updated in-place if:
    * The logits data type is `float32`.
    * The underlying data buffer are contiguous AND on CPU.
    * The data pointer is aligned to 4 bytes.

# Returns

The masked logits. The shape of the returned logits is the same as the input logits. 
The returned logits is the same object as the input logits if the input logits is updated in-place.
Otherwise, a new object with the same type as the input logits is returned.
        """
        logits, ptr, size = _convert_logits_to_slice(logits)
        super().mask_logits(ptr, size)
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
"""
        logits, ptr, size = _convert_logits_to_slice(logits)
        result = super().update_logits(token_id,ptr, size)
        return logits,result
    
    def __copy__(self):
        return super().__copy__()

_try_register_slice_converter("torch", _torch_slice_converter)
_try_register_slice_converter("numpy", _numpy_slice_converter)