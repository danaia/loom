#include <cuda.h>
#include <stdint.h>
#include <unistd.h>

CUresult pqo_cuda_import_probe(
    int ordinal,
    int memory_fd,
    uint64_t allocation_size,
    uint64_t mapped_size,
    int semaphore_fd,
    uint64_t signal_value)
{
    CUresult result = cuInit(0);
    if (result != CUDA_SUCCESS) return result;
    CUdevice device;
    result = cuDeviceGet(&device, ordinal);
    if (result != CUDA_SUCCESS) return result;
    CUcontext context = NULL;
    result = cuCtxCreate(&context, 0, device);
    if (result != CUDA_SUCCESS) return result;
    CUstream stream = NULL;
    CUexternalMemory memory = NULL;
    CUexternalSemaphore semaphore = NULL;

    result = cuStreamCreate(&stream, CU_STREAM_NON_BLOCKING);
    if (result != CUDA_SUCCESS) goto cleanup;
    CUDA_EXTERNAL_MEMORY_HANDLE_DESC memory_desc = {0};
    memory_desc.type = CU_EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_FD;
    memory_desc.handle.fd = memory_fd;
    memory_desc.size = allocation_size;
    result = cuImportExternalMemory(&memory, &memory_desc);
    if (result != CUDA_SUCCESS) goto cleanup;
    memory_fd = -1;
    CUDA_EXTERNAL_MEMORY_BUFFER_DESC buffer_desc = {0};
    buffer_desc.offset = 0;
    buffer_desc.size = mapped_size;
    CUdeviceptr pointer = 0;
    result = cuExternalMemoryGetMappedBuffer(&pointer, memory, &buffer_desc);
    if (result != CUDA_SUCCESS) goto cleanup;

    CUDA_EXTERNAL_SEMAPHORE_HANDLE_DESC semaphore_desc = {0};
    semaphore_desc.type = CU_EXTERNAL_SEMAPHORE_HANDLE_TYPE_TIMELINE_SEMAPHORE_FD;
    semaphore_desc.handle.fd = semaphore_fd;
    result = cuImportExternalSemaphore(&semaphore, &semaphore_desc);
    if (result != CUDA_SUCCESS) goto cleanup;
    semaphore_fd = -1;

    result = cuMemsetD8Async(pointer, 0xA5, (size_t)mapped_size, stream);
    if (result != CUDA_SUCCESS) goto cleanup;
    CUDA_EXTERNAL_SEMAPHORE_SIGNAL_PARAMS signal = {0};
    signal.params.fence.value = signal_value;
    result = cuSignalExternalSemaphoresAsync(&semaphore, &signal, 1, stream);
    if (result != CUDA_SUCCESS) goto cleanup;
    result = cuStreamSynchronize(stream);

cleanup:
    if (memory_fd >= 0) close(memory_fd);
    if (semaphore_fd >= 0) close(semaphore_fd);
    if (semaphore != NULL) cuDestroyExternalSemaphore(semaphore);
    if (memory != NULL) cuDestroyExternalMemory(memory);
    if (stream != NULL) cuStreamDestroy(stream);
    cuCtxDestroy(context);
    return result;
}
