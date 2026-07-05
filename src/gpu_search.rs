use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchHit {
    pub index: usize,
    pub score: u32,
}

pub struct GpuFuzzySearch {
    #[cfg(feature = "gpu-search")]
    min_gpu_candidates: usize,
    #[cfg(feature = "gpu-search")]
    engine: Option<Engine>,
}

impl GpuFuzzySearch {
    pub async fn new(min_gpu_candidates: usize) -> Self {
        #[cfg(not(feature = "gpu-search"))]
        let _ = min_gpu_candidates;
        Self {
            #[cfg(feature = "gpu-search")]
            min_gpu_candidates,
            #[cfg(feature = "gpu-search")]
            engine: Engine::new().await.ok(),
        }
    }

    pub fn gpu_available(&self) -> bool {
        #[cfg(feature = "gpu-search")]
        {
            self.engine.is_some()
        }
        #[cfg(not(feature = "gpu-search"))]
        {
            false
        }
    }

    pub fn rank(&self, query: &str, candidates: &[&str], limit: usize) -> Vec<SearchHit> {
        if query.is_empty() || candidates.is_empty() || limit == 0 {
            return Vec::new();
        }
        #[cfg(feature = "gpu-search")]
        if candidates.len() >= self.min_gpu_candidates
            && query.is_ascii()
            && candidates.iter().all(|candidate| candidate.is_ascii())
            && let Some(engine) = &self.engine
            && let Ok(hits) = engine.rank(query, candidates, limit)
        {
            return hits;
        }
        cpu_rank(query, candidates, limit)
    }
}

pub fn cpu_rank(query: &str, candidates: &[&str], limit: usize) -> Vec<SearchHit> {
    if query.trim().is_empty() || limit == 0 {
        return Vec::new();
    }
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buffer = Vec::new();
    let mut hits: Vec<SearchHit> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            pattern
                .score(Utf32Str::new(candidate, &mut buffer), &mut matcher)
                .map(|score| SearchHit { index, score })
        })
        .collect();
    hits.sort_unstable_by(|a, b| b.score.cmp(&a.score).then_with(|| a.index.cmp(&b.index)));
    hits.truncate(limit);
    hits
}

#[cfg(feature = "gpu-search")]
struct Engine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    max_storage_bytes: usize,
}

#[cfg(feature = "gpu-search")]
impl Engine {
    async fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .map_err(|error| error.to_string())?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("quicklaunch-search"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| error.to_string())?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quicklaunch-search"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("quicklaunch-search"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let max_storage_bytes = device.limits().max_storage_buffer_binding_size as usize;
        Ok(Self {
            device,
            queue,
            pipeline,
            max_storage_bytes,
        })
    }

    fn rank(
        &self,
        query: &str,
        candidates: &[&str],
        limit: usize,
    ) -> Result<Vec<SearchHit>, String> {
        if query.len() > u32::MAX as usize || candidates.len() > u32::MAX as usize {
            return Err("search input exceeds GPU index range".to_string());
        }
        let query_data: Vec<u32> = query.bytes().map(normalize_ascii).collect();
        let mut candidate_data = Vec::new();
        let mut offsets = Vec::with_capacity(candidates.len() * 2);
        for candidate in candidates {
            if candidate.len() > u32::MAX as usize
                || candidate_data
                    .len()
                    .checked_add(candidate.len())
                    .is_none_or(|length| length > u32::MAX as usize)
            {
                return Err("candidate corpus exceeds GPU index range".to_string());
            }
            offsets.push(candidate_data.len() as u32);
            offsets.push(candidate.len() as u32);
            candidate_data.extend(candidate.bytes().map(normalize_ascii));
        }
        let largest_buffer = candidate_data
            .len()
            .max(offsets.len())
            .max(candidates.len())
            .saturating_mul(std::mem::size_of::<u32>());
        if largest_buffer > self.max_storage_bytes {
            return Err("candidate corpus exceeds GPU buffer limits".to_string());
        }
        if candidate_data.is_empty() {
            candidate_data.push(0);
        }
        let params = [query_data.len() as u32, candidates.len() as u32];
        let query_buffer = storage_buffer(&self.device, "query", &query_data);
        let candidate_buffer = storage_buffer(&self.device, "candidates", &candidate_data);
        let offset_buffer = storage_buffer(&self.device, "offsets", &offsets);
        let params_buffer = uniform_buffer(&self.device, "params", &params);
        let output_size = (candidates.len() * std::mem::size_of::<u32>()) as u64;
        let score_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scores"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scores-read"),
            size: output_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let layout = self.pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("quicklaunch-search"),
            layout: &layout,
            entries: &[
                binding(0, &query_buffer),
                binding(1, &candidate_buffer),
                binding(2, &offset_buffer),
                binding(3, &params_buffer),
                binding(4, &score_buffer),
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("quicklaunch-search"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(candidates.len().div_ceil(64) as u32, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&score_buffer, 0, &read_buffer, 0, output_size);
        self.queue.submit([encoder.finish()]);
        let slice = read_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| error.to_string())?;
        receiver
            .recv()
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        let mapped = slice.get_mapped_range();
        let scores: Vec<u32> = bytemuck::cast_slice(&mapped).to_vec();
        drop(mapped);
        read_buffer.unmap();
        let mut hits: Vec<SearchHit> = scores
            .into_iter()
            .enumerate()
            .filter(|(_, score)| *score > 0)
            .map(|(index, score)| SearchHit { index, score })
            .collect();
        hits.sort_unstable_by(|a, b| b.score.cmp(&a.score).then_with(|| a.index.cmp(&b.index)));
        hits.truncate(limit);
        Ok(hits)
    }
}

#[cfg(feature = "gpu-search")]
fn normalize_ascii(byte: u8) -> u32 {
    byte.to_ascii_lowercase() as u32
}

#[cfg(feature = "gpu-search")]
fn storage_buffer(device: &wgpu::Device, label: &str, data: &[u32]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage: wgpu::BufferUsages::STORAGE,
    })
}

#[cfg(feature = "gpu-search")]
fn uniform_buffer(device: &wgpu::Device, label: &str, data: &[u32]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage: wgpu::BufferUsages::UNIFORM,
    })
}

#[cfg(feature = "gpu-search")]
fn binding(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

#[cfg(feature = "gpu-search")]
const SHADER: &str = r#"
@group(0) @binding(0) var<storage, read> query: array<u32>;
@group(0) @binding(1) var<storage, read> candidates: array<u32>;
@group(0) @binding(2) var<storage, read> offsets: array<vec2<u32>>;
struct Params {
    query_len: u32,
    candidate_count: u32,
}

@group(0) @binding(3) var<uniform> params: Params;
@group(0) @binding(4) var<storage, read_write> scores: array<u32>;

fn separator(value: u32) -> bool {
    return value == 32u || value == 45u || value == 46u || value == 47u || value == 95u;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.x;
    if index >= params.candidate_count {
        return;
    }
    let location = offsets[index];
    var query_index = 0u;
    var previous = 0xffffffffu;
    var score = 0u;
    for (var i = 0u; i < location.y && query_index < params.query_len; i = i + 1u) {
        let value = candidates[location.x + i];
        if value == query[query_index] {
            score = score + 100u;
            if previous != 0xffffffffu && previous + 1u == i {
                score = score + 35u;
            }
            if i == 0u || separator(candidates[location.x + i - 1u]) {
                score = score + 25u;
            }
            previous = i;
            query_index = query_index + 1u;
        }
    }
    if query_index == params.query_len {
        scores[index] = score + 64u - min(previous, 64u);
    } else {
        scores[index] = 0u;
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::cpu_rank;

    #[test]
    fn cpu_ranking_prefers_tighter_matches() {
        let candidates = ["firefox", "files and folders", "frost"];
        let hits = cpu_rank("fir", &candidates, 3);
        assert_eq!(hits[0].index, 0);
    }

    #[test]
    fn empty_queries_have_no_hits() {
        assert!(cpu_rank("", &["one"], 4).is_empty());
    }

    #[cfg(feature = "gpu-search")]
    #[test]
    fn gpu_ranking_executes_when_an_adapter_is_available() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let search = runtime.block_on(super::GpuFuzzySearch::new(0));
        if search.gpu_available() {
            let candidates = ["firefox", "files and folders", "frost"];
            let hits = search.rank("fir", &candidates, 3);
            assert_eq!(hits[0].index, 0);
        }
    }
}
