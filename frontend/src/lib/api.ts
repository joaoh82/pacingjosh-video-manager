import type {
  FilterState,
  Video,
  VideoUpdate,
  Production,
  AiSettings,
  AiSettingsUpdate,
  AiGeneration,
  EditJobStatus,
  ProductionEdit,
  StartEditPayload,
  YoutubeCopy,
} from './types';

declare global {
  interface Window {
    __VMAN_API__?: string;
    __TAURI__?: unknown;
    __TAURI_INTERNALS__?: unknown;
  }
}

// Resolution order for the backend base URL:
// 1. `window.__VMAN_API__` — injected by the Tauri shell at startup with the
//    dynamically-assigned embedded-backend port.
// 2. `NEXT_PUBLIC_API_URL` env var — for Next dev mode pointing at a manual
//    `cargo run` backend.
// 3. `http://localhost:8000` fallback — matches the standalone dev default.
//
// The result is normalized to origin-only (no trailing `/api`) so callers can
// safely prepend `/api/...` without producing `/api/api/...`.
function getApiBase(): string {
  const fromWindow =
    typeof window !== 'undefined' && typeof window.__VMAN_API__ === 'string'
      ? window.__VMAN_API__
      : undefined;
  const raw =
    fromWindow || process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8000';
  return raw.replace(/\/$/, '').replace(/\/api$/, '');
}

/**
 * True when running inside the Tauri WebView.
 *
 * In Tauri v2 the convenience global `window.__TAURI__` is only present when
 * `app.withGlobalTauri` is enabled (it is not, here). The reliable signals are
 * `window.__TAURI_INTERNALS__` — the IPC object the runtime always injects
 * before page scripts run — and `window.__VMAN_API__`, which our shell injects
 * at startup. Either one means we're in the desktop app.
 */
export function isTauri(): boolean {
  if (typeof window === 'undefined') return false;
  return (
    typeof window.__TAURI_INTERNALS__ !== 'undefined' ||
    typeof window.__TAURI__ !== 'undefined' ||
    typeof window.__VMAN_API__ === 'string'
  );
}

function apiUrl(path: string): string {
  const base = getApiBase();
  const p = path.startsWith('/') ? path : `/${path}`;
  return `${base}${p}`;
}

async function fetchApi<T>(
  path: string,
  options?: RequestInit
): Promise<T> {
  const url = apiUrl(path);
  const res = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
  });
  if (!res.ok) {
    const text = await res.text();
    let message = text;
    try {
      const json = JSON.parse(text);
      message = json.detail ?? (typeof json.detail === 'string' ? json.detail : text);
    } catch {
      message = res.statusText || text;
    }
    throw new Error(message);
  }
  return res.json() as Promise<T>;
}

// --- Config ---

export interface ConfigResponse {
  configured: boolean;
  video_directory?: string | null;
  supported_formats?: string[];
  thumbnail_count?: number;
}

export async function getConfig(): Promise<ConfigResponse> {
  return fetchApi<ConfigResponse>('/api/config');
}

export async function saveConfig(
  video_directory: string,
  thumbnail_count: number,
  thumbnail_width: number
): Promise<{ status: string; message: string; configured: boolean }> {
  return fetchApi('/api/config', {
    method: 'POST',
    body: JSON.stringify({
      video_directory,
      thumbnail_count,
      thumbnail_width,
    }),
  });
}

// --- Browse folder ---

export interface BrowseFolderResponse {
  success: boolean;
  path?: string;
  message?: string;
}

export async function browseFolder(): Promise<BrowseFolderResponse> {
  return fetchApi<BrowseFolderResponse>('/api/browse-folder');
}

/** Open an OS file picker (used for choosing a background-music track). */
export async function browseFile(): Promise<BrowseFolderResponse> {
  return fetchApi<BrowseFolderResponse>('/api/browse-file');
}

// --- Videos ---

export interface VideoListResponse {
  videos: Video[];
  total: number;
  page: number;
  limit: number;
  pages: number;
}

export async function getVideos(
  filters: FilterState,
  pagination: { page: number; limit: number }
): Promise<VideoListResponse> {
  const params = new URLSearchParams();
  if (filters.search) params.set('search', filters.search);
  if (filters.category) params.set('category', filters.category);
  if (filters.tags.length) params.set('tags', filters.tags.join(','));
  if (filters.production !== null) params.set('production', String(filters.production));
  if (filters.orientation) params.set('orientation', filters.orientation);
  if (filters.dateFrom) params.set('date_from', filters.dateFrom.toISOString());
  if (filters.dateTo) params.set('date_to', filters.dateTo.toISOString());
  params.set('sort', filters.sort);
  params.set('page', String(pagination.page));
  params.set('limit', String(pagination.limit));

  const query = params.toString();
  return fetchApi<VideoListResponse>(`/api/videos${query ? `?${query}` : ''}`);
}

export async function updateVideo(id: number, data: VideoUpdate): Promise<Video> {
  return fetchApi<Video>(`/api/videos/${id}`, {
    method: 'PUT',
    body: JSON.stringify(data),
  });
}

export async function openVideoFolder(id: number): Promise<void> {
  await fetchApi(`/api/videos/${id}/open-folder`, { method: 'POST' });
}

// --- Bulk update ---

export interface BulkUpdateRequest {
  video_ids: number[];
  category?: string | null;
  location?: string | null;
  notes?: string | null;
  add_tags?: string[] | null;
  remove_tags?: string[] | null;
  add_production_ids?: number[] | null;
  remove_production_ids?: number[] | null;
}

export interface BulkUpdateResponse {
  updated: number;
  message: string;
}

export async function bulkUpdateVideos(
  request: BulkUpdateRequest
): Promise<BulkUpdateResponse> {
  return fetchApi<BulkUpdateResponse>('/api/videos/bulk-update', {
    method: 'POST',
    body: JSON.stringify(request),
  });
}

// --- Categories & tags ---

export interface CategoryItem {
  name: string;
  count: number;
}

export async function getCategories(): Promise<CategoryItem[]> {
  return fetchApi<CategoryItem[]>('/api/tags/categories');
}

export interface TagWithCountItem {
  id: number;
  name: string;
  count: number;
}

export async function getTags(): Promise<TagWithCountItem[]> {
  return fetchApi<TagWithCountItem[]>('/api/tags');
}

// --- Productions ---

export async function getProductions(): Promise<Production[]> {
  return fetchApi<Production[]>('/api/productions');
}

export interface ProductionPayload {
  title: string;
  platform?: string | null;
  link?: string | null;
  is_published: boolean;
}

export async function createProduction(data: ProductionPayload): Promise<Production> {
  return fetchApi<Production>('/api/productions', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function updateProduction(id: number, data: ProductionPayload): Promise<Production> {
  return fetchApi<Production>(`/api/productions/${id}`, {
    method: 'PUT',
    body: JSON.stringify(data),
  });
}

export async function deleteProduction(id: number): Promise<void> {
  await fetchApi(`/api/productions/${id}`, { method: 'DELETE' });
}

// --- Statistics ---

export async function getStatistics(): Promise<Record<string, unknown>> {
  return fetchApi<Record<string, unknown>>('/api/videos/stats/summary');
}

// --- Stream & thumbnails ---

export function getStreamUrl(videoId: number): string {
  return apiUrl(`/api/stream/${videoId}`);
}

export function getThumbnailUrl(videoId: number, index: number = 0): string {
  return apiUrl(`/api/thumbnails/${videoId}/${index}`);
}

// --- Scan ---

export interface ScanStartResponse {
  status: string;
  scan_id: string;
  message: string;
}

export async function startScan(options: {
  directory: string;
  save_config: boolean;
}): Promise<ScanStartResponse> {
  return fetchApi<ScanStartResponse>('/api/scan', {
    method: 'POST',
    body: JSON.stringify(options),
  });
}

export interface ScanStatus {
  scan_id: string;
  status: string;
  total: number;
  processed: number;
  successful: number;
  failed: number;
  skipped: number;
  current_file: string;
  errors: string[];
  start_time: string;
  end_time: string | null;
  elapsed_seconds: number;
  eta_seconds: number | null;
}

export async function getScanStatus(scanId: string): Promise<ScanStatus> {
  return fetchApi<ScanStatus>(`/api/scan/status/${scanId}`);
}

export async function rescanDirectory(): Promise<ScanStartResponse> {
  return fetchApi<ScanStartResponse>('/api/scan/rescan', {
    method: 'POST',
  });
}

// --- AI content generation (desktop only) ---

export async function getAiSettings(): Promise<AiSettings> {
  return fetchApi<AiSettings>('/api/ai/settings');
}

export async function saveAiSettings(
  data: AiSettingsUpdate
): Promise<{ status: string; message: string }> {
  return fetchApi('/api/ai/settings', {
    method: 'PUT',
    body: JSON.stringify(data),
  });
}

/** Returns the saved generation for a video, or null if none exists yet. */
export async function getAiGeneration(videoId: number): Promise<AiGeneration | null> {
  return fetchApi<AiGeneration | null>(`/api/ai/generation/${videoId}`);
}

/** Runs transcription + copy generation. Can take 30s+; show a spinner. */
export async function generateAiContent(
  videoId: number,
  regenerate = false
): Promise<AiGeneration> {
  return fetchApi<AiGeneration>(`/api/ai/generate/${videoId}`, {
    method: 'POST',
    body: JSON.stringify({ regenerate }),
  });
}

// --- Video edit pipeline (desktop only) ---

export interface StartEditResponse {
  status: string;
  job_id: string;
  message: string;
}

/**
 * Start the "Edit & Create Video" pipeline for a production. Transcribes every
 * take, asks the LLM to assemble the best cut from the script, then stitches
 * the final clip with ffmpeg. Returns a job id to poll with getEditStatus.
 */
export async function startProductionEdit(
  productionId: number,
  data: StartEditPayload
): Promise<StartEditResponse> {
  return fetchApi<StartEditResponse>(`/api/productions/${productionId}/edit`, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

/** Poll live progress for a running (or finished) edit job. */
export async function getEditStatus(jobId: string): Promise<EditJobStatus> {
  return fetchApi<EditJobStatus>(`/api/edit/status/${jobId}`);
}

/** The latest persisted edit result for a production, or null if none. */
export async function getProductionEdit(
  productionId: number
): Promise<ProductionEdit | null> {
  return fetchApi<ProductionEdit | null>(`/api/productions/${productionId}/edit`);
}

/** Full edit history for a production (newest first). */
export async function getProductionEdits(
  productionId: number
): Promise<ProductionEdit[]> {
  return fetchApi<ProductionEdit[]>(`/api/productions/${productionId}/edits`);
}

/** Reveal the latest final video for a production in the OS file browser. */
export async function revealEditOutput(productionId: number): Promise<void> {
  await fetchApi(`/api/productions/${productionId}/edit/reveal`, { method: 'POST' });
}

/** Reveal a specific run's final video in the OS file browser. */
export async function revealEditFile(editId: number): Promise<void> {
  await fetchApi(`/api/edits/${editId}/reveal`, { method: 'POST' });
}

/** Delete a run: removes its DB row and its files (video, EDL, version folder). */
export async function deleteEdit(editId: number): Promise<void> {
  await fetchApi(`/api/edits/${editId}`, { method: 'DELETE' });
}

/**
 * Re-render a run into a new version with timeline edits applied — `mute` is the
 * list of music regions (seconds, final timeline) to remove. Reuses the saved
 * cut/transcription (no extra cost). Returns a job id to poll with getEditStatus.
 */
export async function rerenderEdit(
  editId: number,
  mute: { start: number; end: number }[]
): Promise<StartEditResponse> {
  return fetchApi<StartEditResponse>(`/api/edits/${editId}/rerender`, {
    method: 'POST',
    body: JSON.stringify({ mute }),
  });
}

/**
 * Generate (or fetch cached) long-form YouTube copy — title options,
 * description, tags, and thumbnail text — from a finished run's transcript.
 * Pass regenerate=true to force a fresh generation.
 */
export async function generateEditCopy(
  editId: number,
  regenerate = false
): Promise<YoutubeCopy> {
  return fetchApi<YoutubeCopy>(`/api/edits/${editId}/copy`, {
    method: 'POST',
    body: JSON.stringify({ regenerate }),
  });
}

// --- Thumbnail builder ---

/** Grab a 1280x720 still frame from a run's final video at `t` seconds. */
export async function fetchEditFrame(editId: number, t: number): Promise<Blob> {
  const res = await fetch(apiUrl(`/api/edits/${editId}/frame?t=${t}`));
  if (!res.ok) throw new Error((await res.text()) || 'Failed to grab frame');
  return res.blob();
}

/** AI-restyle a still frame via Gemini's image model (requires a Gemini key). */
export async function restyleEditFrame(
  editId: number,
  t: number,
  prompt?: string
): Promise<Blob> {
  const res = await fetch(apiUrl(`/api/edits/${editId}/restyle`), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ t, prompt }),
  });
  if (!res.ok) {
    let msg = await res.text();
    try {
      msg = JSON.parse(msg).detail ?? msg;
    } catch {
      /* keep text */
    }
    throw new Error(msg || 'AI restyle failed');
  }
  return res.blob();
}

/** Save a finished thumbnail (base64/data-URL PNG) next to the run's video. */
export async function saveEditThumbnail(
  editId: number,
  imageBase64: string
): Promise<{ path: string }> {
  return fetchApi<{ path: string }>(`/api/edits/${editId}/thumbnail`, {
    method: 'POST',
    body: JSON.stringify({ image: imageBase64 }),
  });
}
