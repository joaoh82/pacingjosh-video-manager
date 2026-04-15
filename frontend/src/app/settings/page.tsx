'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import { getConfig, saveConfig, browseFolder } from '@/lib/api';

export default function SettingsPage() {
  const router = useRouter();
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isBrowsing, setIsBrowsing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const [videoDirectory, setVideoDirectory] = useState('');
  const [thumbnailCount, setThumbnailCount] = useState(5);
  const [thumbnailWidth, setThumbnailWidth] = useState(320);

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const config = await getConfig();
      setVideoDirectory(config.video_directory || '');
      setThumbnailCount(config.thumbnail_count || 5);
      // Note: thumbnail_width is not in the config response, but we can add it if needed
    } catch (err: any) {
      setError(err.message || 'Failed to load settings');
    } finally {
      setIsLoading(false);
    }
  };

  const handleBrowseFolder = async () => {
    setIsBrowsing(true);
    setError(null);

    try {
      const response = await browseFolder();
      if (response.success && response.path) {
        setVideoDirectory(response.path);
      } else {
        if (response.message !== 'No folder selected') {
          setError(response.message || 'Failed to open folder picker');
        }
      }
    } catch (err: any) {
      setError(err.message || 'Failed to open folder picker');
    } finally {
      setIsBrowsing(false);
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsSaving(true);
    setError(null);
    setSuccessMessage(null);

    try {
      await saveConfig(videoDirectory, thumbnailCount, thumbnailWidth);
      setSuccessMessage('Settings saved successfully!');
      setTimeout(() => setSuccessMessage(null), 3000);
    } catch (err: any) {
      setError(err.message || 'Failed to save settings');
    } finally {
      setIsSaving(false);
    }
  };

  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-center">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600 mx-auto mb-4" />
          <p className="text-gray-600 dark:text-gray-400">Loading settings...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900">
      <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div>
            <h1 className="text-3xl font-bold text-gray-900 dark:text-white">
              Settings
            </h1>
            <p className="mt-2 text-gray-600 dark:text-gray-400">
              Configure your video manager settings
            </p>
          </div>
          <button
            onClick={() => router.push('/')}
            className="btn btn-secondary"
          >
            ← Back to Videos
          </button>
        </div>

        {/* Settings Form */}
        <form onSubmit={handleSave} className="space-y-6">
          {/* Video Directory */}
          <div className="card">
            <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-4">
              Video Directory
            </h2>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Directory Path
                </label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={videoDirectory}
                    onChange={(e) => setVideoDirectory(e.target.value)}
                    className="input flex-1"
                    placeholder="/path/to/your/videos"
                    required
                  />
                  <button
                    type="button"
                    onClick={handleBrowseFolder}
                    disabled={isBrowsing}
                    className="btn btn-secondary whitespace-nowrap"
                  >
                    {isBrowsing ? '⏳ Opening...' : '📁 Browse...'}
                  </button>
                </div>
                <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                  The directory where your video files are stored. This directory will be scanned recursively.
                </p>
              </div>
            </div>
          </div>

          {/* Thumbnail Settings */}
          <div className="card">
            <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-4">
              Thumbnail Settings
            </h2>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Number of Thumbnails per Video
                </label>
                <input
                  type="number"
                  min="1"
                  max="10"
                  value={thumbnailCount}
                  onChange={(e) => setThumbnailCount(parseInt(e.target.value))}
                  className="input w-32"
                />
                <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                  Number of thumbnails to generate for each video (1-10). More thumbnails take more disk space but provide better preview.
                </p>
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Thumbnail Width (pixels)
                </label>
                <input
                  type="number"
                  min="160"
                  max="640"
                  step="80"
                  value={thumbnailWidth}
                  onChange={(e) => setThumbnailWidth(parseInt(e.target.value))}
                  className="input w-32"
                />
                <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                  Width of generated thumbnails in pixels (160-640). Height is calculated automatically to maintain aspect ratio.
                </p>
              </div>
            </div>
          </div>

          {/* Messages */}
          {error && (
            <div className="p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
              <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
            </div>
          )}

          {successMessage && (
            <div className="p-4 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg">
              <p className="text-sm text-green-600 dark:text-green-400">{successMessage}</p>
            </div>
          )}

          {/* Actions */}
          <div className="flex items-center justify-between">
            <button
              type="button"
              onClick={() => router.push('/')}
              className="btn btn-secondary"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSaving}
              className="btn btn-primary"
            >
              {isSaving ? 'Saving...' : 'Save Settings'}
            </button>
          </div>
        </form>

        {/* Additional Info */}
        <div className="mt-8 p-4 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg">
          <h3 className="font-medium text-blue-900 dark:text-blue-100 mb-2">
            💡 After Changing Settings
          </h3>
          <ul className="text-sm text-blue-800 dark:text-blue-200 space-y-1 list-disc list-inside">
            <li>If you change the video directory, click &ldquo;Rescan&rdquo; on the main page to index videos from the new location</li>
            <li>Thumbnail settings only apply to newly scanned videos</li>
            <li>To regenerate thumbnails for existing videos, delete them and rescan</li>
          </ul>
        </div>
      </div>
    </div>
  );
}
