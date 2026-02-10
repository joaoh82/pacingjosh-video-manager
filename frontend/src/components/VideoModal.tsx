'use client';

import { useState, useEffect } from 'react';
import { Video, VideoUpdate, Production } from '@/lib/types';
import { getStreamUrl, updateVideo, getProductions } from '@/lib/api';
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
  const [currentVideo, setCurrentVideo] = useState<Video>(video);
  const [allProductions, setAllProductions] = useState<Production[]>([]);
  const [formData, setFormData] = useState({
    category: video.metadata?.category || '',
    location: video.metadata?.location || '',
    notes: video.metadata?.notes || '',
    tags: video.tags.map((t) => t.name).join(', '),
    production_ids: (video.productions || []).map((p) => p.id),
  });

  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
      // Reset form data and current video when video prop changes
      setCurrentVideo(video);
      setFormData({
        category: video.metadata?.category || '',
        location: video.metadata?.location || '',
        notes: video.metadata?.notes || '',
        tags: video.tags.map((t) => t.name).join(', '),
        production_ids: (video.productions || []).map((p) => p.id),
      });
      setIsEditing(false);
      // Load productions for the picker
      getProductions()
        .then(setAllProductions)
        .catch(() => {});
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
        production_ids: formData.production_ids,
      };

      const updatedVideo = await updateVideo(video.id, updateData);
      setCurrentVideo(updatedVideo);
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

  const toggleProduction = (prodId: number) => {
    setFormData((prev) => ({
      ...prev,
      production_ids: prev.production_ids.includes(prodId)
        ? prev.production_ids.filter((id) => id !== prodId)
        : [...prev.production_ids, prodId],
    }));
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
                      rows={4}
                      placeholder="Add notes about this video..."
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      Productions
                    </label>
                    {allProductions.length === 0 ? (
                      <p className="text-sm text-gray-500 dark:text-gray-400 italic">
                        No productions available. Create one from the Productions manager.
                      </p>
                    ) : (
                      <div className="max-h-48 overflow-y-auto space-y-2 border border-gray-300 dark:border-gray-600 rounded-lg p-3">
                        {allProductions.map((prod) => (
                          <label
                            key={prod.id}
                            className="flex items-center space-x-2 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-700 p-1 rounded"
                          >
                            <input
                              type="checkbox"
                              checked={formData.production_ids.includes(prod.id)}
                              onChange={() => toggleProduction(prod.id)}
                              className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                            />
                            <span className="text-sm text-gray-700 dark:text-gray-300 flex-1 truncate">
                              {prod.title}
                            </span>
                          </label>
                        ))}
                      </div>
                    )}
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
                      {currentVideo.metadata?.category && (
                        <div>
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Category:</span>
                          <p className="text-gray-900 dark:text-white mt-1">{currentVideo.metadata.category}</p>
                        </div>
                      )}

                      {currentVideo.metadata?.location && (
                        <div>
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Location:</span>
                          <p className="text-gray-900 dark:text-white mt-1">{currentVideo.metadata.location}</p>
                        </div>
                      )}

                      {currentVideo.tags.length > 0 && (
                        <div>
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Tags:</span>
                          <div className="flex flex-wrap gap-2 mt-2">
                            {currentVideo.tags.map((tag) => (
                              <span key={tag.id} className="badge badge-primary">
                                {tag.name}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}

                      {currentVideo.metadata?.notes && (
                        <div>
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Notes:</span>
                          <p className="text-gray-900 dark:text-white mt-1 whitespace-pre-wrap">
                            {currentVideo.metadata.notes}
                          </p>
                        </div>
                      )}

                      {(currentVideo.productions || []).length > 0 && (
                        <div>
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">Productions:</span>
                          <ul className="mt-2 space-y-1">
                            {currentVideo.productions.map((prod) => (
                              <li key={prod.id} className="text-sm flex items-center gap-2">
                                {prod.link ? (
                                  <a
                                    href={prod.link}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="text-primary-600 dark:text-primary-400 hover:underline"
                                  >
                                    {prod.title}
                                  </a>
                                ) : (
                                  <span className="text-gray-900 dark:text-white">{prod.title}</span>
                                )}
                                {prod.platform && (
                                  <span className="text-xs px-1.5 py-0.5 rounded bg-gray-200 dark:bg-gray-600 text-gray-600 dark:text-gray-300">
                                    {prod.platform}
                                  </span>
                                )}
                                {!prod.is_published && (
                                  <span className="text-xs px-1.5 py-0.5 rounded bg-yellow-100 dark:bg-yellow-900/40 text-yellow-700 dark:text-yellow-400">
                                    Draft
                                  </span>
                                )}
                              </li>
                            ))}
                          </ul>
                        </div>
                      )}

                      {!currentVideo.metadata?.category && !currentVideo.metadata?.location &&
                       currentVideo.tags.length === 0 && !currentVideo.metadata?.notes &&
                       (currentVideo.productions || []).length === 0 && (
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
