//! Real hardware detection and live resource sampling. Nothing here is mocked: values that cannot
//! be measured on the current machine are reported as `None`.

pub mod gpu;
pub mod sampler;
pub mod system;

pub use sampler::{merge_summaries, summarize, Sampler};
pub use system::{detect_system, primary_gpu_label};
