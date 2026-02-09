'use client';

import { useState } from 'react';
import { bulkUpdateVideos, BulkUpdateRequest } from '@/lib/api';

interface BulkActionsProps {
  selectedCount: number;
  selectedVideoIds: number[];
  onClearSelection: () => void;
  onUpdate: () => void;
}

export default function BulkActions({
  selectedCount,
  selectedVideoIds,
  onClearSelection,
  onUpdate,
}: BulkActionsProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [category, setCategory] = useState('');
  const [addTags, setAddTags] = useState('');
  const [removeTags, setRemoveTags] = useState('');

  if (selectedCount === 0) return null;

  const handleBulkUpdate = async () => {
    if (selectedVideoIds.length === 0) return;

    setIsProcessing(true);
    try {
      const request: BulkUpdateRequest = {
        video_ids: selectedVideoIds,
      };

      if (category) request.category = category;
      if (addTags) {
        request.add_tags = addTags.split(',').map((t) => t.trim()).filter(Boolean);
      }
      if (removeTags) {
        request.remove_tags = removeTags.split(',').map((t) => t.trim()).filter(Boolean);
      }

      await bulkUpdateVideos(request);

      // Reset form
      setCategory('');
      setAddTags('');
      setRemoveTags('');
      setIsOpen(false);
      onClearSelection();
      onUpdate();
    } catch (error) {
      console.error('Bulk update failed:', error);
      alert('Failed to update videos');
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <div className="fixed bottom-0 left-0 right-0 bg-white dark:bg-gray-800 border-t dark:border-gray-700 shadow-lg z-40">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
              {selectedCount} video{selectedCount !== 1 ? 's' : ''} selected
            </span>
            <button
              onClick={onClearSelection}
              className="text-sm text-gray-600 hover:text-gray-900 dark:text-gray-400 dark:hover:text-gray-100"
            >
              Clear
            </button>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => setIsOpen(!isOpen)}
              className="btn btn-primary"
            >
              Bulk Edit
            </button>
          </div>
        </div>

        {/* Bulk Edit Form */}
        {isOpen && (
          <div className="mt-4 p-4 bg-gray-50 dark:bg-gray-700 rounded-lg">
            <h3 className="font-medium text-gray-900 dark:text-white mb-4">
              Update Selected Videos
            </h3>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Set Category
                </label>
                <input
                  type="text"
                  value={category}
                  onChange={(e) => setCategory(e.target.value)}
                  className="input"
                  placeholder="e.g., Running"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Add Tags (comma-separated)
                </label>
                <input
                  type="text"
                  value={addTags}
                  onChange={(e) => setAddTags(e.target.value)}
                  className="input"
                  placeholder="e.g., morning, 5k"
                />
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Remove Tags (comma-separated)
                </label>
                <input
                  type="text"
                  value={removeTags}
                  onChange={(e) => setRemoveTags(e.target.value)}
                  className="input"
                  placeholder="e.g., old-tag"
                />
              </div>
            </div>

            <div className="flex justify-end gap-2 mt-4">
              <button
                onClick={() => setIsOpen(false)}
                disabled={isProcessing}
                className="btn btn-secondary"
              >
                Cancel
              </button>
              <button
                onClick={handleBulkUpdate}
                disabled={isProcessing}
                className="btn btn-primary"
              >
                {isProcessing ? 'Updating...' : 'Apply Changes'}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
