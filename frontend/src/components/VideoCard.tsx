'use client';

import { useState } from 'react';
import { Video } from '@/lib/types';
import { getThumbnailUrl, openVideoFolder } from '@/lib/api';
import { format } from 'date-fns';

interface VideoCardProps {
  video: Video;
  onSelect?: (videoId: number) => void;
  isSelected?: boolean;
  onClick?: (video: Video) => void;
}

export default function VideoCard({
  video,
  onSelect,
  isSelected = false,
  onClick,
}: VideoCardProps) {
  const [currentThumbIndex, setCurrentThumbIndex] = useState(0);
  const [imageError, setImageError] = useState(false);

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    if (video.thumbnail_count === 0) return;

    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = x / rect.width;
    const index = Math.floor(percentage * video.thumbnail_count);
    setCurrentThumbIndex(Math.max(0, Math.min(index, video.thumbnail_count - 1)));
  };

  const handleMouseLeave = () => {
    setCurrentThumbIndex(0);
  };

  const formatDuration = (seconds?: number | null) => {
    if (!seconds) return '--:--';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);

    if (hours > 0) {
      return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    }
    return `${minutes}:${secs.toString().padStart(2, '0')}`;
  };

  const formatFileSize = (bytes?: number | null) => {
    if (!bytes) return '--';
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }

    return `${size.toFixed(1)} ${units[unitIndex]}`;
  };

  const thumbnailUrl = video.thumbnail_count > 0 && !imageError
    ? getThumbnailUrl(video.id, currentThumbIndex)
    : null;

  return (
    <div
      className={`group relative bg-white dark:bg-gray-800 rounded-lg shadow-md overflow-hidden cursor-pointer transition-all duration-200 hover:shadow-xl hover:scale-105 ${
        isSelected ? 'ring-4 ring-primary-500' : ''
      }`}
      onClick={() => onClick?.(video)}
    >
      {/* Selection checkbox */}
      {onSelect && (
        <div
          className="absolute top-2 left-2 z-10"
          onClick={(e) => {
            e.stopPropagation();
            onSelect(video.id);
          }}
        >
          <input
            type="checkbox"
            checked={isSelected}
            onChange={() => {}}
            className="w-5 h-5 rounded border-gray-300 text-primary-600 focus:ring-primary-500 cursor-pointer"
          />
        </div>
      )}

      {/* Thumbnail */}
      <div
        className="relative aspect-video bg-gray-200 dark:bg-gray-700 overflow-hidden"
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
      >
        {thumbnailUrl ? (
          <img
            src={thumbnailUrl}
            alt={video.filename}
            className="w-full h-full object-cover"
            onError={() => setImageError(true)}
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center">
            <svg
              className="w-16 h-16 text-gray-400"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z"
              />
            </svg>
          </div>
        )}

        {/* Duration overlay */}
        {video.duration && (
          <div className="absolute bottom-2 right-2 bg-black bg-opacity-75 text-white text-xs px-2 py-1 rounded">
            {formatDuration(video.duration)}
          </div>
        )}

        {/* Resolution badge */}
        {video.resolution && (
          <div className="absolute top-2 right-2 bg-black bg-opacity-75 text-white text-xs px-2 py-1 rounded">
            {video.resolution}
          </div>
        )}
      </div>

      {/* Info section */}
      <div className="p-4">
        <div className="flex items-center gap-1 mb-2">
          <h3 className="font-semibold text-gray-900 dark:text-white truncate flex-1">
            {video.filename}
          </h3>
          <button
            onClick={(e) => {
              e.stopPropagation();
              openVideoFolder(video.id).catch(() => {});
            }}
            className="flex-shrink-0 p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 opacity-0 group-hover:opacity-100 transition-opacity"
            title="Show in folder"
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
            </svg>
          </button>
        </div>

        <div className="flex items-center justify-between text-sm text-gray-600 dark:text-gray-400 mb-2">
          <span>{formatFileSize(video.file_size)}</span>
          {video.created_date && (
            <span>{format(new Date(video.created_date), 'MMM d, yyyy')}</span>
          )}
        </div>

        {/* Tags */}
        {video.tags && video.tags.length > 0 && (
          <div className="flex flex-wrap gap-1 mb-2">
            {video.tags.slice(0, 3).map((tag) => (
              <span
                key={tag.id}
                className="badge badge-primary text-xs"
              >
                {tag.name}
              </span>
            ))}
            {video.tags.length > 3 && (
              <span className="badge badge-gray text-xs">
                +{video.tags.length - 3}
              </span>
            )}
          </div>
        )}

        {/* Productions */}
        {video.productions && video.productions.length > 0 && (
          <div className="flex flex-wrap gap-1 mb-2">
            {video.productions.slice(0, 2).map((prod) => (
              <span
                key={prod.id}
                className="inline-flex items-center gap-1 text-xs px-1.5 py-0.5 rounded bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300"
              >
                {prod.title}
                {prod.platform && (
                  <span className="text-purple-500 dark:text-purple-400">
                    ({prod.platform})
                  </span>
                )}
              </span>
            ))}
            {video.productions.length > 2 && (
              <span className="badge badge-gray text-xs">
                +{video.productions.length - 2}
              </span>
            )}
          </div>
        )}

        {/* Category */}
        {video.metadata?.category && (
          <div className="text-sm text-gray-600 dark:text-gray-400">
            <span className="font-medium">Category:</span> {video.metadata.category}
          </div>
        )}
      </div>

      {/* Hover overlay */}
      <div className="absolute inset-0 bg-black bg-opacity-0 group-hover:bg-opacity-10 transition-all duration-200 pointer-events-none" />
    </div>
  );
}
