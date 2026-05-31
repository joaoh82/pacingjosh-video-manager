'use client';

import { useState, useEffect } from 'react';
import { Video, VideoUpdate, Production, AiGeneration } from '@/lib/types';
import {
  getStreamUrl,
  updateVideo,
  getProductions,
  openVideoFolder,
  isTauri,
  getAiGeneration,
  generateAiContent,
} from '@/lib/api';
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

  // AI content generation (desktop only, portrait videos only)
  const aiEligible = isTauri() && video.orientation === 'portrait';
  const [aiGen, setAiGen] = useState<AiGeneration | null>(null);
  const [aiLoading, setAiLoading] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [transcriptOpen, setTranscriptOpen] = useState(false);
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

      // Load any previously-generated AI content for portrait videos.
      setAiGen(null);
      setAiError(null);
      setTranscriptOpen(false);
      if (isTauri() && video.orientation === 'portrait') {
        getAiGeneration(video.id)
          .then((gen) => setAiGen(gen))
          .catch(() => {});
      }
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

  const handleGenerate = async (regenerate: boolean) => {
    setAiLoading(true);
    setAiError(null);
    try {
      const gen = await generateAiContent(video.id, regenerate);
      setAiGen(gen);
    } catch (err: any) {
      setAiError(err.message || 'AI generation failed');
    } finally {
      setAiLoading(false);
    }
  };

  const copyToClipboard = async (key: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedKey(key);
      setTimeout(() => setCopiedKey(null), 1500);
    } catch {
      // Clipboard may be unavailable; silently ignore.
    }
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
                  <div className="flex items-center justify-between">
                    <span className="text-gray-600 dark:text-gray-400 text-sm">Path:</span>
                    <button
                      onClick={() => openVideoFolder(video.id).catch(() => alert('Could not open folder'))}
                      className="inline-flex items-center gap-1 text-xs text-primary-600 dark:text-primary-400 hover:underline"
                      title="Show in folder"
                    >
                      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
                      </svg>
                      Open folder
                    </button>
                  </div>
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

          {/* AI Content panel — desktop only, portrait videos only */}
          {aiEligible && (
            <div className="px-6 pb-6">
              <div className="card">
                <div className="flex items-center justify-between mb-4">
                  <div>
                    <h3 className="font-semibold text-gray-900 dark:text-white">
                      AI Content
                    </h3>
                    <p className="text-xs text-gray-500 dark:text-gray-400">
                      Thumbnail text & platform descriptions from the video transcript.
                      {aiGen?.generated_at && (
                        <> Generated {format(new Date(aiGen.generated_at), 'MMM d, yyyy HH:mm')}.</>
                      )}
                    </p>
                  </div>
                  <button
                    onClick={() => handleGenerate(!!aiGen)}
                    disabled={aiLoading}
                    className="btn btn-primary text-sm whitespace-nowrap"
                  >
                    {aiLoading
                      ? 'Generating…'
                      : aiGen
                      ? 'Regenerate'
                      : 'Generate from transcript'}
                  </button>
                </div>

                {aiLoading && (
                  <div className="flex items-center gap-3 text-sm text-gray-600 dark:text-gray-400">
                    <div className="animate-spin rounded-full h-5 w-5 border-b-2 border-primary-600" />
                    Transcribing audio and generating copy — this can take a little while…
                  </div>
                )}

                {aiError && !aiLoading && (
                  <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                    <p className="text-sm text-red-600 dark:text-red-400">{aiError}</p>
                  </div>
                )}

                {!aiLoading && !aiError && !aiGen && (
                  <p className="text-sm text-gray-500 dark:text-gray-400 italic">
                    No AI content yet. Click “Generate from transcript” to create thumbnail
                    text and Instagram, TikTok, and YouTube Short descriptions.
                  </p>
                )}

                {aiGen && !aiLoading && (
                  <div className="space-y-4">
                    {/* Thumbnail text suggestions */}
                    {aiGen.thumbnail_text.length > 0 && (
                      <div>
                        <span className="text-sm font-medium text-gray-600 dark:text-gray-400">
                          Thumbnail text ideas
                        </span>
                        <ul className="mt-2 space-y-2">
                          {aiGen.thumbnail_text.map((t, i) => (
                            <li
                              key={i}
                              className="flex items-center justify-between gap-2 bg-gray-50 dark:bg-gray-700/50 rounded px-3 py-2"
                            >
                              <span className="text-sm text-gray-900 dark:text-white">{t}</span>
                              <button
                                onClick={() => copyToClipboard(`thumb-${i}`, t)}
                                className="text-xs text-primary-600 dark:text-primary-400 hover:underline whitespace-nowrap"
                              >
                                {copiedKey === `thumb-${i}` ? 'Copied!' : 'Copy'}
                              </button>
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}

                    {/* Platform descriptions */}
                    {[
                      { key: 'instagram', label: 'Instagram description', value: aiGen.instagram_description },
                      { key: 'tiktok', label: 'TikTok description', value: aiGen.tiktok_description },
                      { key: 'youtube-title', label: 'YouTube Short title', value: aiGen.youtube_short_title },
                      { key: 'youtube', label: 'YouTube Short description', value: aiGen.youtube_short_description },
                    ].map(({ key, label, value }) =>
                      value ? (
                        <div key={key}>
                          <div className="flex items-center justify-between mb-1">
                            <span className="text-sm font-medium text-gray-600 dark:text-gray-400">
                              {label}
                            </span>
                            <button
                              onClick={() => copyToClipboard(key, value)}
                              className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
                            >
                              {copiedKey === key ? 'Copied!' : 'Copy'}
                            </button>
                          </div>
                          <p className="text-sm text-gray-900 dark:text-white whitespace-pre-wrap bg-gray-50 dark:bg-gray-700/50 rounded px-3 py-2">
                            {value}
                          </p>
                        </div>
                      ) : null
                    )}

                    {/* YouTube keyword tags */}
                    {aiGen.youtube_short_tags.length > 0 && (
                      <div>
                        <div className="flex items-center justify-between mb-1">
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">
                            YouTube tags
                          </span>
                          <button
                            onClick={() =>
                              copyToClipboard('yt-tags', aiGen.youtube_short_tags.join(', '))
                            }
                            className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
                          >
                            {copiedKey === 'yt-tags' ? 'Copied!' : 'Copy all'}
                          </button>
                        </div>
                        <div className="flex flex-wrap gap-2">
                          {aiGen.youtube_short_tags.map((t, i) => (
                            <span
                              key={i}
                              className="badge bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-200 text-xs"
                            >
                              {t}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* Hashtags */}
                    {aiGen.hashtags.length > 0 && (
                      <div>
                        <div className="flex items-center justify-between mb-1">
                          <span className="text-sm font-medium text-gray-600 dark:text-gray-400">
                            Hashtags
                          </span>
                          <button
                            onClick={() => copyToClipboard('hashtags', aiGen.hashtags.join(' '))}
                            className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
                          >
                            {copiedKey === 'hashtags' ? 'Copied!' : 'Copy all'}
                          </button>
                        </div>
                        <div className="flex flex-wrap gap-2">
                          {aiGen.hashtags.map((h, i) => (
                            <span key={i} className="badge badge-primary text-xs">
                              {h}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* Transcript (collapsible) */}
                    {aiGen.transcript && (
                      <div>
                        <button
                          onClick={() => setTranscriptOpen((o) => !o)}
                          className="text-sm font-medium text-gray-600 dark:text-gray-400 hover:underline"
                        >
                          {transcriptOpen ? '▼' : '▶'} Transcript
                        </button>
                        {transcriptOpen && (
                          <div className="mt-2">
                            <div className="flex justify-end mb-1">
                              <button
                                onClick={() => copyToClipboard('transcript', aiGen.transcript || '')}
                                className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
                              >
                                {copiedKey === 'transcript' ? 'Copied!' : 'Copy'}
                              </button>
                            </div>
                            <p className="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap bg-gray-50 dark:bg-gray-700/50 rounded px-3 py-2 max-h-48 overflow-y-auto">
                              {aiGen.transcript}
                            </p>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
