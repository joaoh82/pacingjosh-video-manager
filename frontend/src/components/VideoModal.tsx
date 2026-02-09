'use client';

import { useState, useEffect } from 'react';
import { Video, VideoUpdate } from '@/lib/types';
import { getStreamUrl, updateVideo } from '@/lib/api';
import { format } from 'date-fns';

interface VideoModalProps {
  video: Video;
  isOpen: boolean;
  onClose: () => void;
  onUpdate?: () => void;
}

export default function VideoModal({
  video,
  isOpen,
  onClose,
  onUpdate,
}: VideoModalProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [formData, setFormData] = useState({
    category: video.metadata?.category || '',
    location: video.metadata?.location || '',
    notes: video.metadata?.notes || '',
    tags: video.tags.map((t) => t.name).join(', '),
  });

  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
      // Reset form data when video changes
      setFormData({
        category: video.metadata?.category || '',
        location: video.metadata?.location || '',
        notes: video.metadata?.notes || '',
        tags: video.tags.map((t) => t.name).join(', '),
      });
      setIsEditing(false);
    } else {
      document.body.style.overflow = 'unset';
    }

    return () => {
      document.body.style.overflow = 'unset';
    };
  }, [isOpen, video]);

  if (!isOpen) return null;

  const handleSave = async () => {
    setIsSaving(true);
    try {
      const updateData: VideoUpdate = {
        category: formData.category || null,
        location: formData.location || null,
        notes: formData.notes || null,
        tags: formData.tags
          ? formData.tags.split(',').map((t) => t.trim()).filter(Boolean)
          : [],
      };

      await updateVideo(video.id, updateData);
      setIsEditing(false);
      onUpdate?.();
    } catch (error) {
      console.error('Failed to update video:', error);
      alert('Failed to update video metadata');
    } finally {
      setIsSaving(false);
    }
  };

  const formatFileSize = (bytes?: number | null) => {
    if (!bytes) return 'Unknown';
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }

    return `${size.toFixed(2)} ${units[unitIndex]}`;
  };

  const formatDuration = (seconds?: number | null) => {
    if (!seconds) return 'Unknown';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);

    if (hours > 0) {
      return `${hours}h ${minutes}m ${secs}s`;
    }
    return `${minutes}m ${secs}s`;
  };

  return (
    <div className="fixed inset-0 z-50 overflow-y-auto">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black bg-opacity-50 transition-opacity"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="flex min-h-full items-center justify-center p-4">
        <div className="relative w-full max-w-6xl bg-white dark:bg-gray-800 rounded-lg shadow-xl overflow-hidden">
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b dark:border-gray-700">
            <h2 className="text-xl font-semibold text-gray-900 dark:text-white truncate pr-4">
              {video.filename}
            </h2>
            <div className="flex items-center gap-2">
              {isEditing ? (
                <>
                  <button
                    onClick={handleSave}
                    disabled={isSaving}
                    className="btn btn-primary text-sm"
                  >
                    {isSaving ? 'Saving...' : 'Save'}
                  </button>
                  <button
                    onClick={() => setIsEditing(false)}
                    disabled={isSaving}
                    className="btn btn-secondary text-sm"
                  >
                    Cancel
                  </button>
                </>
              ) : (
                <button
                  onClick={() => setIsEditing(true)}
                  className="btn btn-secondary text-sm"
                >
                  Edit
                </button>
              )}
              <button
                onClick={onClose}
                className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
              >
                <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          </div>

          {/* Content */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 p-6">
            {/* Video Player */}
            <div className="space-y-4">
              <div className="aspect-video bg-black rounded-lg overflow-hidden">
                <video
                  className="w-full h-full"
                  controls
                  src={getStreamUrl(video.id)}
                >
                  Your browser does not support video playback.
                </video>
              </div>

              {/* Technical Info */}
              <div className="card">
                <h3 className="font-semibold text-gray-900 dark:text-white mb-3">
                  Technical Details
                </h3>
                <div className="grid grid-cols-2 gap-3 text-sm">
                  <div>
                    <span className="text-gray-600 dark:text-gray-400">Duration:</span>
                    <span className="ml-2 text-gray-900 dark:text-white">{formatDuration(video.duration)}</span>
                  </div>
                  <div>
                    <span className="text-gray-600 dark:text-gray-400">File Size:</span>
                    <span className="ml-2 text-gray-900 dark:text-white">{formatFileSize(video.file_size)}</span>
                  </div>
                  <div>
                    <span className="text-gray-600 dark:text-gray-400">Resolution:</span>
                    <span className="ml-2 text-gray-900 dark:text-white">{video.resolution || 'Unknown'}</span>
                  </div>
                  <div>
                    <span className="text-gray-600 dark:text-gray-400">FPS:</span>
                    <span className="ml-2 text-gray-900 dark:text-white">
                      {video.fps ? video.fps.toFixed(2) : 'Unknown'}
                    </span>
                  </div>
                  <div>
                    <span className="text-gray-600 dark:text-gray-400">Codec:</span>
                    <span className="ml-2 text-gray-900 dark:text-white">{video.codec || 'Unknown'}</span>
                  </div>
                  <div>
                    <span className="text-gray-600 dark:text-gray-400">Created:</span>
                    <span className="ml-2 text-gray-900 dark:text-white">
                      {video.created_date ? format(new Date(video.created_date), 'MMM d, yyyy') : 'Unknown'}
                    </span>
                  </div>
                </div>
                <div className="mt-3 pt-3 border-t dark:border-gray-700">
                  <span className="text-gray-600 dark:text-gray-400 text-sm">Path:</span>
                  <p className="text-xs text-gray-900 dark:text-white break-all mt-1">{video.file_path}</p>
                </div>
              </div>
            </div>

            {/* Metadata */}
            <div className="space-y-4">
              {isEditing ? (
                // Edit Form
                <div className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Category
                    </label>
                    <input
                      type="text"
                      value={formData.category}
                      onChange={(e) => setFormData({ ...formData, category: e.target.value })}
                      className="input"
                      placeholder="e.g., Running, Trail, Road"
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Location
                    </label>
                    <input
                      type="text"
                      value={formData.location}
                      onChange={(e) => setFormData({ ...formData, location: e.target.value })}
                      className="input"
                      placeholder="e.g., Central Park, NYC"
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Tags (comma-separated)
                    </label>
                    <input
                      type="text"
                      value={formData.tags}
                      onChange={(e) => setFormData({ ...formData, tags: e.target.value })}
                      className="input"
                      placeholder="e.g., morning, 5k, training"
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      Notes
                    </label>
                    <textarea
                      value={formData.notes}
                      onChange={(e) => setFormData({ ...formData, notes: e.target.value })}
                      className="input"
                      rows={6}
                      placeholder="Add notes about this video..."
                    />
                  </div>
                </div>
              ) : (
                // Display Metadata
                <div className="space-y-4">
                  <div className="card">
                    <h3 className="font-semibold text-gray-900 dark:text-white mb-3">
                      Metadata
                    </h3>

                    <div className="space-y-3">
                      {video.metadata?.category && (
                        <div>
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Category:</span>
                          <p className="text-gray-900 dark:text-white mt-1">{video.metadata.category}</p>
                        </div>
                      )}

                      {video.metadata?.location && (
                        <div>
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Location:</span>
                          <p className="text-gray-900 dark:text-white mt-1">{video.metadata.location}</p>
                        </div>
                      )}

                      {video.tags.length > 0 && (
                        <div>
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Tags:</span>
                          <div className="flex flex-wrap gap-2 mt-2">
                            {video.tags.map((tag) => (
                              <span key={tag.id} className="badge badge-primary">
                                {tag.name}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}

                      {video.metadata?.notes && (
                        <div>
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Notes:</span>
                          <p className="text-gray-900 dark:text-white mt-1 whitespace-pre-wrap">
                            {video.metadata.notes}
                          </p>
                        </div>
                      )}

                      {!video.metadata?.category && !video.metadata?.location &&
                       video.tags.length === 0 && !video.metadata?.notes && (
                        <p className="text-gray-500 dark:text-gray-400 text-sm italic">
                          No metadata available. Click Edit to add information.
                        </p>
                      )}
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
