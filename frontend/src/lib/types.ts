/** Filter state for the video list (search, category, tags, date range, sort). */
export interface FilterState {
  search: string;
  category: string;
  tags: string[];
  production: number | null;
  dateFrom: Date | null;
  dateTo: Date | null;
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
  metadata?: VideoMetadata | null;
  tags: VideoTag[];
  productions: Production[];
}

/** Payload for updating a single video's metadata. */
export interface VideoUpdate {
  category?: string | null;
  location?: string | null;
  notes?: string | null;
  tags?: string[];
  production_ids?: number[];
}
