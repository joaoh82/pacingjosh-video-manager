'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { startScan, browseFolder } from '@/lib/api';
import Scanner from '@/components/Scanner';

export default function SetupPage() {
  const router = useRouter();
  const [directory, setDirectory] = useState('');
  const [isScanning, setIsScanning] = useState(false);
  const [scanId, setScanId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isBrowsing, setIsBrowsing] = useState(false);

  const handleBrowseFolder = async () => {
    console.log('Browse button clicked');
    setIsBrowsing(true);
    setError(null);

    try {
      console.log('Calling browseFolder API...');
      const response = await browseFolder();
      console.log('Browse response:', response);

      if (response.success && response.path) {
        console.log('Setting directory to:', response.path);
        setDirectory(response.path);
        setError(null);
      } else {
        console.log('No folder selected or error:', response.message);
        if (response.message !== 'No folder selected') {
          setError(response.message || 'Failed to open folder picker');
        }
      }
    } catch (err: any) {
      console.error('Browse error:', err);
      setError(err.message || 'Failed to open folder picker');
    } finally {
      setIsBrowsing(false);
    }
  };

  const handleStartScan = async () => {
    if (!directory.trim()) {
      setError('Please enter a directory path');
      return;
    }

    setError(null);
    setIsScanning(true);

    try {
      const response = await startScan({
        directory: directory.trim(),
        save_config: true,
      });

      setScanId(response.scan_id);
    } catch (err: any) {
      setError(err.message || 'Failed to start scan');
      setIsScanning(false);
    }
  };

  const handleScanComplete = () => {
    // Redirect to main page after scan completes
    setTimeout(() => {
      router.push('/');
    }, 2000);
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-4">
      <div className="w-full max-w-2xl">
        <div className="text-center mb-8">
          <h1 className="text-4xl font-bold text-gray-900 dark:text-white mb-2">
            Welcome to Video Manager
          </h1>
          <p className="text-lg text-gray-600 dark:text-gray-400">
            Let's get started by indexing your videos
          </p>
        </div>

        {!isScanning ? (
          <div className="card">
            <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-4">
              Select Video Directory
            </h2>
            <p className="text-gray-600 dark:text-gray-400 mb-6">
              Enter the path to the directory containing your video files. The
              application will scan this directory and all subdirectories for
              supported video formats.
            </p>

            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Directory Path
                </label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={directory}
                    onChange={(e) => {
                      setDirectory(e.target.value);
                      setError(null);
                    }}
                    placeholder="/Users/username/Videos or /home/user/Videos"
                    className="input flex-1"
                    onKeyDown={(e) => e.key === 'Enter' && handleStartScan()}
                  />
                  <button
                    type="button"
                    onClick={handleBrowseFolder}
                    className="btn btn-secondary whitespace-nowrap"
                    disabled={isScanning || isBrowsing}
                  >
                    {isBrowsing ? '⏳ Opening...' : '📁 Browse...'}
                  </button>
                </div>
                <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                  💡 <strong>Tip:</strong> Click "Browse..." to select a folder using your system's file picker, or type the path manually.
                  <br />
                  • macOS: <code className="bg-gray-100 dark:bg-gray-800 px-1 rounded">/Users/username/Movies</code>
                  <br />
                  • Linux: <code className="bg-gray-100 dark:bg-gray-800 px-1 rounded">/home/username/Videos</code>
                  <br />
                  • Windows: <code className="bg-gray-100 dark:bg-gray-800 px-1 rounded">C:\Users\username\Videos</code>
                </p>
                {error && (
                  <p className="mt-2 text-sm text-red-600 dark:text-red-400">
                    {error}
                  </p>
                )}
              </div>

              <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
                <h3 className="font-medium text-blue-900 dark:text-blue-100 mb-2">
                  Supported Formats
                </h3>
                <p className="text-sm text-blue-800 dark:text-blue-200">
                  .mp4, .mov, .avi, .mkv, .webm, .flv, .wmv
                </p>
              </div>

              <button
                onClick={handleStartScan}
                className="w-full btn btn-primary py-3 text-lg"
              >
                Start Scanning
              </button>
            </div>
          </div>
        ) : scanId ? (
          <Scanner scanId={scanId} onComplete={handleScanComplete} />
        ) : (
          <div className="card">
            <div className="animate-pulse flex items-center gap-3">
              <div className="w-12 h-12 bg-gray-300 dark:bg-gray-600 rounded-full" />
              <div className="flex-1 space-y-2">
                <div className="h-4 bg-gray-300 dark:bg-gray-600 rounded w-1/2" />
                <div className="h-3 bg-gray-300 dark:bg-gray-600 rounded w-3/4" />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
