import types
from .kbnf import InternalEngine
import typing
_slice_converters = []

def _try_register_slice_converter(module_name:str,
                        obtain_converter:typing.Callable[[types.ModuleType],
                                                        typing.Callable[[typing.Any],
                                                                        typing.Optional[typing.Tuple[typing.Any,int,int]]]]):
    try:
        module = __import__(module_name)
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
        logits, ptr, size = _convert_logits_to_slice(logits)
        result = super().mask_logits(ptr, size)
        return logits, result
    
    def update_logits(self, token_id:int, logits):
        logits, ptr, size = _convert_logits_to_slice(logits)
        result = super().update_logits(token_id,ptr, size)
        return logits,result

_try_register_slice_converter("torch", _torch_slice_converter)
_try_register_slice_converter("numpy", _numpy_slice_converter)