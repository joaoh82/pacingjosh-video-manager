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

export type ProductionType = 'long' | 'short';

export interface Production {
  id: number;
  title: string;
  platform?: string | null;
  link?: string | null;
  is_published: boolean;
  production_type: ProductionType;
  published_at?: string | null;
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
  /** Provider used for AI thumbnail restyling: "gemini" | "openai". */
  image_provider: string;
  /** Model id for AI image generation/editing. */
  image_model: string;
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
  image_provider?: string;
  image_model?: string;
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
  /** Whether voice enhancement (noise removal) was applied to this clip. */
  enhanced?: boolean;
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
  /** Whether voice enhancement (noise removal) was applied to this clip. */
  enhanced?: boolean;
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

/** An overlay snippet placed on the final timeline (e.g. a Subscribe bug).
 * Carries the display fields plus the full spec so the timeline editor can
 * rehydrate and re-send it on a re-render. */
export interface TimelineOverlay {
  label?: string | null;
  filename?: string | null;
  /** Absolute path to the snippet file (used to re-send on re-render). */
  path?: string;
  /** Where the snippet appears on the final timeline (seconds). */
  start: number;
  /** When it ends (start + duration). */
  end: number;
  duration: number;
  position: string;
  /** Background colour keyed out ("" → none). */
  chroma_color?: string;
  similarity?: number;
  blend?: number;
  scale?: number;
  opacity?: number;
}

/** Editor-style timeline: clips laid end-to-end, speech intervals, music. */
export interface EditTimeline {
  duration: number;
  clips: TimelineClip[];
  speech: TimelineSpeech[];
  /** Intervals where the music is ducked (speech + any user-muted regions). */
  duck?: TimelineSpeech[];
  /** Music regions the user removed on the saved run (kept sticky on reopen). */
  muted?: TimelineSpeech[];
  /** Intervals where the music fades in/out (applied on the saved run). */
  fades?: TimelineSpeech[];
  /** Overlay snippets dropped into the pauses (read-only on the timeline). */
  overlays?: TimelineOverlay[];
  music: TimelineMusic;
}

/** A pending per-clip edit collected on the timeline for a re-render. */
export interface ClipEdit {
  /** The clip `order` (1-based, as in the EDL) this edit targets. */
  order: number;
  /** Drop the clip from the re-rendered cut. */
  remove?: boolean;
  /** New source range start (seconds into the take); omit to keep. */
  source_start?: number;
  /** New source range end (seconds into the take); omit to keep. */
  source_end?: number;
  /** Apply voice enhancement to this clip. */
  enhance?: boolean;
}

/** A pending music-region edit collected on the timeline for a re-render. */
export interface MusicEdit {
  start: number;
  end: number;
  /** "remove" ducks the music away; "fade" ramps it in/out. */
  action: 'remove' | 'fade';
}

/** A timeline edit plan proposed by the AI assistant. */
export interface TimelineAiPlan {
  clips: ClipEdit[];
  music: MusicEdit[];
  explanation: string;
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

/** Generated long-form YouTube copy for a finished run. */
export interface YoutubeCopy {
  titles: string[];
  description: string;
  tags: string[];
  thumbnail_texts: string[];
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
  /** Generated YouTube copy, if any. */
  copy?: YoutubeCopy | null;
  /** Saved thumbnail builder state, if a thumbnail was saved for this run. */
  thumbnail?: ThumbnailSpec | null;
}

/** A renderable text treatment for a thumbnail caption (canvas overlay). */
export interface ThumbnailTextStyle {
  /** Solid text color (used when `gradient` is null). */
  fill: string;
  /** Optional top→bottom gradient fill. */
  gradient: { from: string; to: string } | null;
  outlineColor: string;
  outlineWidth: number;
  shadowColor: string;
  shadowBlur: number;
  shadowOffsetY: number;
  /** Optional colored band drawn behind the text. */
  highlight: { color: string; textColor: string } | null;
}

/** Persisted thumbnail builder state — enough to rebuild and re-edit it. */
export interface ThumbnailSpec {
  text: string;
  fontSize: number;
  uppercase: boolean;
  align: 'left' | 'center' | 'right';
  /** Horizontal anchor, 0–1 of width. */
  posX: number;
  /** Vertical anchor, 0–1 of height. */
  posY: number;
  /** Frame time (seconds) the background still was grabbed from. */
  frameTime: number;
  /** Whether the saved background was AI-restyled. */
  restyled: boolean;
  style: ThumbnailTextStyle;
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
  /** Take (video) ids to clean up with "Enhance voice" (noise removal). */
  enhance_voice?: number[];
  /** Voice-enhancement intensity, 0.0–1.0 (how aggressively to remove noise). */
  enhance_voice_intensity?: number;
  /** Overlay snippets (e.g. a "Subscribe" bug) to drop into the pauses. */
  overlays?: OverlaySpecPayload[];
}

/** One overlay snippet to composite onto the final video (sent on start-edit).
 * A transparent GIF/PNG (its native alpha is used). Auto-placed in the longest
 * pause where no one is talking. */
export interface OverlaySpecPayload {
  /** Path to the overlay file (transparent GIF/image). */
  path: string;
  /** Display label (e.g. "Subscribe"). */
  label?: string;
  /** Optional background colour to chroma-key out (e.g. "0xFFFFFF"). Empty/omit
   * → use the snippet's native transparency (the normal case). */
  chroma_color?: string;
  /** Scale factor (1.0 = original size). */
  scale?: number;
  /** Opacity (0..1). */
  opacity?: number;
  /** Position preset ("center", "bottom", "bottom_right", …). */
  position?: string;
  /** On-screen duration (seconds); omit to use the snippet's own length. */
  duration?: number;
  /** Explicit start time on the final timeline (seconds); omit to auto-place. */
  start?: number;
}

/** A built-in overlay snippet offered by the backend (e.g. the Subscribe bug). */
export interface BuiltinOverlay {
  id: string;
  label: string;
  path: string;
}
