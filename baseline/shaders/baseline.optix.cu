// CUDA baseline view entry. The first executable CUDA runtime will wire this
// entry to the RTX/OptiX or CUDA raster path that consumes render.* streams.

extern "C" __global__ void baseline_pipeline()
{
}
