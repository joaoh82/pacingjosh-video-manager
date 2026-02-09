'use client';

import { useMemo } from 'react';
import { Video } from '@/lib/types';
import VideoCard from './VideoCard';

interface VideoGridProps {
  videos: Video[];
  isLoading?: boolean;
  selectedVideos?: Set<number>;
  onVideoSelect?: (videoId: number) => void;
  onVideoClick?: (video: Video) => void;
  emptyMessage?: string;
}

interface VideoGroup {
  key: string;
  label: string;
  videos: Video[];
}

function groupVideosByMonth(videos: Video[]): VideoGroup[] {
  const groups = new Map<string, Video[]>();

  for (const video of videos) {
    const dateStr = video.created_date || video.indexed_date;
    const date = new Date(dateStr);
    const year = date.getFullYear();
    const month = date.getMonth();

    if (isNaN(year)) {
      const key = 'unknown';
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(video);
      continue;
    }

    const key = `${year}-${String(month).padStart(2, '0')}`;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(video);
  }

  const monthNames = [
    'January', 'February', 'March', 'April', 'May', 'June',
    'July', 'August', 'September', 'October', 'November', 'December',
  ];

  return Array.from(groups.entries()).map(([key, vids]) => {
    if (key === 'unknown') {
      return { key, label: 'Unknown Date', videos: vids };
    }
    const [year, monthStr] = key.split('-');
    const monthIndex = parseInt(monthStr, 10);
    return {
      key,
      label: `${monthNames[monthIndex]} ${year}`,
      videos: vids,
    };
  });
}

export default function VideoGrid({
  videos,
  isLoading = false,
  selectedVideos = new Set(),
  onVideoSelect,
  onVideoClick,
  emptyMessage = 'No videos found',
}: VideoGridProps) {
  const groups = useMemo(() => groupVideosByMonth(videos), [videos]);

  if (isLoading) {
    return (
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
        {Array.from({ length: 8 }).map((_, i) => (
          <div
            key={i}
            className="bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden animate-pulse"
          >
            <div className="aspect-video bg-gray-300 dark:bg-gray-700" />
            <div className="p-4 space-y-3">
              <div className="h-4 bg-gray-300 dark:bg-gray-700 rounded w-3/4" />
              <div className="h-3 bg-gray-300 dark:bg-gray-700 rounded w-1/2" />
              <div className="flex gap-2">
                <div className="h-5 bg-gray-300 dark:bg-gray-700 rounded w-16" />
                <div className="h-5 bg-gray-300 dark:bg-gray-700 rounded w-16" />
              </div>
            </div>
          </div>
        ))}
      </div>
    );
  }

  if (videos.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-gray-500 dark:text-gray-400">
        <svg
          className="w-24 h-24 mb-4"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z"
          />
        </svg>
        <p className="text-lg font-medium">{emptyMessage}</p>
        <p className="text-sm mt-2">Try adjusting your filters or search terms</p>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      {groups.map((group) => (
        <section key={group.key}>
          <div className="flex items-center gap-3 mb-4">
            <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
              {group.label}
            </h2>
            <span className="text-sm text-gray-500 dark:text-gray-400">
              ({group.videos.length} video{group.videos.length !== 1 ? 's' : ''})
            </span>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
            {group.videos.map((video) => (
              <VideoCard
                key={video.id}
                video={video}
                isSelected={selectedVideos.has(video.id)}
                onSelect={onVideoSelect}
                onClick={onVideoClick}
              />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
