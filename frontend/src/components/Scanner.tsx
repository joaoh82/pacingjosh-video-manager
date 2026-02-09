'use client';

import { useEffect, useState } from 'react';
import { ScanStatus, getScanStatus } from '@/lib/api';

interface ScannerProps {
  scanId: string;
  onComplete?: () => void;
}

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${Math.round(seconds)}s`;
  const mins = Math.floor(seconds / 60);
  const secs = Math.round(seconds % 60);
  if (mins < 60) return `${mins}m ${secs}s`;
  const hrs = Math.floor(mins / 60);
  const remainMins = mins % 60;
  return `${hrs}h ${remainMins}m`;
}

export default function Scanner({ scanId, onComplete }: ScannerProps) {
  const [status, setStatus] = useState<ScanStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let interval: NodeJS.Timeout;

    const fetchStatus = async () => {
      try {
        const data = await getScanStatus(scanId);
        setStatus(data);

        if (data.status === 'completed' || data.status === 'failed') {
          clearInterval(interval);
          if (data.status === 'completed' && onComplete) {
            setTimeout(onComplete, 1000);
          }
        }
      } catch (err) {
        setError('Failed to fetch scan status');
        clearInterval(interval);
      }
    };

    fetchStatus();
    interval = setInterval(fetchStatus, 1000);

    return () => clearInterval(interval);
  }, [scanId, onComplete]);

  if (error) {
    return (
      <div className="card bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800">
        <div className="flex items-center gap-3">
          <svg className="w-6 h-6 text-red-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <div>
            <p className="font-medium text-red-900 dark:text-red-100">Error</p>
            <p className="text-sm text-red-700 dark:text-red-300">{error}</p>
          </div>
        </div>
      </div>
    );
  }

  if (!status) {
    return (
      <div className="card">
        <div className="animate-pulse flex items-center gap-3">
          <div className="w-12 h-12 bg-gray-300 dark:bg-gray-600 rounded-full" />
          <div className="flex-1 space-y-2">
            <div className="h-4 bg-gray-300 dark:bg-gray-600 rounded w-1/2" />
            <div className="h-3 bg-gray-300 dark:bg-gray-600 rounded w-3/4" />
          </div>
        </div>
      </div>
    );
  }

  const progress = status.total > 0 ? (status.processed / status.total) * 100 : 0;
  const isComplete = status.status === 'completed';
  const isFailed = status.status === 'failed';
  const rate = status.elapsed_seconds > 0 && status.processed > 0
    ? (status.processed / status.elapsed_seconds)
    : 0;

  return (
    <div className={`card ${isFailed ? 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800' : ''}`}>
      <div className="flex items-start gap-4">
        {/* Status Icon */}
        <div className="flex-shrink-0">
          {isComplete ? (
            <div className="w-12 h-12 bg-green-100 dark:bg-green-900 rounded-full flex items-center justify-center">
              <svg className="w-6 h-6 text-green-600 dark:text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
            </div>
          ) : isFailed ? (
            <div className="w-12 h-12 bg-red-100 dark:bg-red-900 rounded-full flex items-center justify-center">
              <svg className="w-6 h-6 text-red-600 dark:text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </div>
          ) : (
            <div className="w-12 h-12 bg-primary-100 dark:bg-primary-900 rounded-full flex items-center justify-center">
              <svg className="w-6 h-6 text-primary-600 dark:text-primary-400 animate-spin" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
              </svg>
            </div>
          )}
        </div>

        {/* Status Info */}
        <div className="flex-1 min-w-0">
          <h3 className="font-semibold text-gray-900 dark:text-white mb-1">
            {isComplete
              ? 'Scan Complete!'
              : isFailed
              ? 'Scan Failed'
              : 'Scanning Videos...'}
          </h3>

          {/* Progress Bar */}
          {!isComplete && !isFailed && (
            <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2 mb-2">
              <div
                className="bg-primary-600 h-2 rounded-full transition-all duration-300"
                style={{ width: `${progress}%` }}
              />
            </div>
          )}

          {/* Progress percentage and time info */}
          {!isComplete && !isFailed && status.total > 0 && (
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-gray-600 dark:text-gray-400 mb-2">
              <span className="font-medium text-gray-900 dark:text-white">
                {Math.round(progress)}%
              </span>
              <span>
                Elapsed: {formatDuration(status.elapsed_seconds)}
              </span>
              {status.eta_seconds != null && status.processed > 0 && (
                <span>
                  Remaining: ~{formatDuration(status.eta_seconds)}
                </span>
              )}
              {rate > 0 && (
                <span>
                  {rate >= 1
                    ? `${rate.toFixed(1)} videos/s`
                    : `${(1 / rate).toFixed(1)}s/video`}
                </span>
              )}
            </div>
          )}

          {/* Completed summary with elapsed */}
          {isComplete && (
            <div className="text-sm text-gray-600 dark:text-gray-400 mb-2">
              Completed in {formatDuration(status.elapsed_seconds)}
            </div>
          )}

          {/* Stats */}
          <div className="flex flex-wrap gap-4 text-sm text-gray-600 dark:text-gray-400 mb-2">
            <span>
              <strong className="text-gray-900 dark:text-white">{status.processed}</strong> /{' '}
              {status.total} processed
            </span>
            <span className="text-green-600 dark:text-green-400">
              {status.successful} successful
            </span>
            {status.skipped > 0 && (
              <span className="text-yellow-600 dark:text-yellow-400">
                {status.skipped} skipped
              </span>
            )}
            {status.failed > 0 && (
              <span className="text-red-600 dark:text-red-400">
                {status.failed} failed
              </span>
            )}
          </div>

          {/* Current File */}
          {status.current_file && !isComplete && !isFailed && (
            <p className="text-xs text-gray-500 dark:text-gray-400 truncate">
              Processing: {status.current_file}
            </p>
          )}

          {/* Errors */}
          {status.errors.length > 0 && (
            <details className="mt-2">
              <summary className="text-sm text-red-600 dark:text-red-400 cursor-pointer">
                View Errors ({status.errors.length})
              </summary>
              <div className="mt-2 text-xs text-red-600 dark:text-red-400 space-y-1 max-h-32 overflow-y-auto">
                {status.errors.map((error, i) => (
                  <div key={i} className="p-2 bg-red-50 dark:bg-red-900/20 rounded">
                    {error}
                  </div>
                ))}
              </div>
            </details>
          )}
        </div>
      </div>
    </div>
  );
}
