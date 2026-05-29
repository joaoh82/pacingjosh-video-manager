/** Filter state for the video list (search, category, tags, date range, sort). */
export interface FilterState {
  search: string;
  category: string;
  tags: string[];
  production: number | null;
  dateFrom: Date | null;
  dateTo: Date | null;
  /** '' = any, otherwise 'portrait' | 'landscape' | 'square'. */
  orientation: string;
  sort: SortOption;
}

export type SortOption =
  | 'date_desc'
  | 'date_asc'
  | 'name_asc'
  | 'name_desc'
  | 'duration_desc'
  | 'duration_asc'
  | 'size_desc'
  | 'size_asc';

export interface Category {
  name: string;
  count: number;
}

export interface TagWithCount {
  id: number;
  name: string;
  count: number;
}

export interface Production {
  id: number;
  title: string;
  platform?: string | null;
  link?: string | null;
  is_published: boolean;
  video_count?: number;
}

export interface VideoMetadata {
  category?: string | null;
  location?: string | null;
  notes?: string | null;
}

export interface VideoTag {
  id: number;
  name: string;
}

export type VideoOrientation = 'landscape' | 'portrait' | 'square';

export interface Video {
  id: number;
  filename: string;
  file_path: string;
  duration?: number | null;
  file_size?: number | null;
  resolution?: string | null;
  fps?: number | null;
  codec?: string | null;
  created_date?: string | null;
  indexed_date: string;
  thumbnail_count: number;
  /** Derived from resolution: 'landscape' | 'portrait' | 'square'. */
  orientation?: VideoOrientation | string | null;
  metadata?: VideoMetadata | null;
  tags: VideoTag[];
  productions: Production[];
}

/** AI/LLM provider configuration (API keys are write-only — never returned). */
export interface AiSettings {
  text_provider: string;
  text_model: string;
  transcription_provider: string;
  transcription_model: string;
  gemini_api_key_set: boolean;
  openai_api_key_set: boolean;
  anthropic_api_key_set: boolean;
}

/** Payload for saving AI settings. Blank/omitted keys are left unchanged. */
export interface AiSettingsUpdate {
  text_provider?: string;
  text_model?: string;
  transcription_provider?: string;
  transcription_model?: string;
  gemini_api_key?: string;
  openai_api_key?: string;
  anthropic_api_key?: string;
}

/** AI-generated content for a single (portrait) video. */
export interface AiGeneration {
  video_id: number;
  transcript?: string | null;
  thumbnail_text: string[];
  instagram_description?: string | null;
  tiktok_description?: string | null;
  youtube_short_description?: string | null;
  hashtags: string[];
  provider?: string | null;
  model?: string | null;
  generated_at: string;
}

/** Payload for updating a single video's metadata. */
export interface VideoUpdate {
  category?: string | null;
  location?: string | null;
  notes?: string | null;
  tags?: string[];
  production_ids?: number[];
}
