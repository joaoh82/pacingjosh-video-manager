import type { FilterState, Video, VideoUpdate, Production } from './types';

// Normalize base to origin only (no trailing /api) so paths like /api/browse-folder never double up
function getApiBase(): string {
  const raw = typeof window !== 'undefined'
    ? (process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8000')
    : process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8000';
  return raw.replace(/\/$/, '').replace(/\/api$/, '');
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
