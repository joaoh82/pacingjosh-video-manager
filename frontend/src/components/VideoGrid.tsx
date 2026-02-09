'use client';

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

export default function VideoGrid({
  videos,
  isLoading = false,
  selectedVideos = new Set(),
  onVideoSelect,
  onVideoClick,
  emptyMessage = 'No videos found',
}: VideoGridProps) {
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
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
      {videos.map((video) => (
        <VideoCard
          key={video.id}
          video={video}
          isSelected={selectedVideos.has(video.id)}
          onSelect={onVideoSelect}
          onClick={onVideoClick}
        />
      ))}
    </div>
  );
}
