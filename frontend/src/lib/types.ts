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
  elevenlabs_api_key_set: boolean;
  /** The editable copy-generation prompt currently in use. */
  system_prompt: string;
  /** The built-in default prompt, for offering a "reset to default". */
  default_system_prompt: string;
  /** The editable video-edit pipeline planning prompt currently in use. */
  edit_prompt: string;
  /** The built-in default edit prompt, for offering a "reset to default". */
  default_edit_prompt: string;
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
  elevenlabs_api_key?: string;
  /** Omitted leaves it unchanged; an empty string resets it to the default. */
  system_prompt?: string;
  /** Omitted leaves it unchanged; an empty string resets it to the default. */
  edit_prompt?: string;
}

/** AI-generated content for a single (portrait) video. */
export interface AiGeneration {
  video_id: number;
  transcript?: string | null;
  thumbnail_text: string[];
  instagram_description?: string | null;
  tiktok_description?: string | null;
  youtube_short_title?: string | null;
  youtube_short_description?: string | null;
  youtube_short_tags: string[];
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

// --- Video edit pipeline (desktop) ---

/** One clip in the assembled edit decision list. */
export interface EditClip {
  order: number;
  video_id: number;
  filename: string;
  start: number;
  end: number;
  duration: number;
  reason?: string | null;
}

/** One clip placed on the assembled timeline. */
export interface TimelineClip {
  order: number;
  video_id: number;
  filename: string;
  /** Position on the final timeline (seconds). */
  start: number;
  end: number;
  /** Range taken from the source take (seconds). */
  source_start: number;
  source_end: number;
}

/** A speech interval on the final timeline (where the music ducks). */
export interface TimelineSpeech {
  start: number;
  end: number;
}

export interface TimelineMusic {
  present: boolean;
  name?: string | null;
  /** Music level when no one is talking (0..1). */
  full_volume: number;
  /** Music level while talking (0..1). */
  duck_volume: number;
}

/** Editor-style timeline: clips laid end-to-end, speech intervals, music. */
export interface EditTimeline {
  duration: number;
  clips: TimelineClip[];
  speech: TimelineSpeech[];
  music: TimelineMusic;
}

/** The edit decision list produced by the pipeline. */
export interface EditDecisionList {
  production_id: number;
  production_title: string;
  generated_at: string;
  transcription_provider?: string;
  text_provider?: string;
  text_model?: string;
  captions?: boolean;
  music?: string | null;
  clips: EditClip[];
  output?: string | null;
  timeline?: EditTimeline;
}

/** Live progress for a running edit pipeline job. */
export interface EditJobStatus {
  job_id: string;
  production_id: number;
  status: 'in_progress' | 'completed' | 'failed';
  stage: string;
  message: string;
  processed: number;
  total: number;
  logs: string[];
  error?: string | null;
  edl?: EditDecisionList | null;
  output_path?: string | null;
  edl_path?: string | null;
  elapsed_seconds: number;
  start_time: string;
  end_time?: string | null;
}

/** A persisted edit result for a production. */
export interface ProductionEdit {
  id: number;
  production_id: number;
  status: string;
  script?: string | null;
  instructions?: string | null;
  edl?: EditDecisionList | null;
  output_path?: string | null;
  edl_path?: string | null;
  error?: string | null;
  logs: string[];
  transcription_provider?: string | null;
  text_provider?: string | null;
  text_model?: string | null;
  created_at: string;
}

/** Payload for starting an edit pipeline run. */
export interface StartEditPayload {
  script: string;
  instructions?: string;
  output_dir?: string;
  output_name?: string;
  captions?: boolean;
  music_path?: string;
  /** Music volume when no one is talking (0.0–1.0). */
  music_volume?: number;
  /** Music volume while the voice is talking — the ducked level (0.0–1.0). */
  music_duck_volume?: number;
  /** Only swell the music in pauses longer than this many seconds. */
  music_min_gap?: number;
  /** Remove long silences/filler within clips (tighten the cut). */
  tighten?: boolean;
  /** When tightening, cut silence/filler gaps longer than this many seconds. */
  tighten_gap?: number;
}
