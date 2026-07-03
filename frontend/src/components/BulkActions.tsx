'use client';

import { useState, useEffect } from 'react';
import { Production, Video } from '@/lib/types';
import { bulkUpdateVideos, BulkUpdateRequest, getProductions, deleteVideo } from '@/lib/api';

interface BulkActionsProps {
  selectedCount: number;
  selectedVideoIds: number[];
  /** Full video objects for the selection (used by single-video delete). */
  selectedVideos?: Video[];
  onClearSelection: () => void;
  onUpdate: () => void;
}

export default function BulkActions({
  selectedCount,
  selectedVideoIds,
  selectedVideos = [],
  onClearSelection,
  onUpdate,
}: BulkActionsProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [category, setCategory] = useState('');
  const [addTags, setAddTags] = useState('');
  const [removeTags, setRemoveTags] = useState('');
  const [allProductions, setAllProductions] = useState<Production[]>([]);
  const [addProductionIds, setAddProductionIds] = useState<number[]>([]);
  const [removeProductionIds, setRemoveProductionIds] = useState<number[]>([]);

  // Single-video delete (with a library-only vs delete-from-disk choice).
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      getProductions().then(setAllProductions).catch(() => {});
    }
  }, [isOpen]);

  // Reset the delete panel whenever the selection changes.
  useEffect(() => {
    setDeleteOpen(false);
    setDeleteError(null);
  }, [selectedVideoIds.join(',')]); // eslint-disable-line react-hooks/exhaustive-deps

  if (selectedCount === 0) return null;

  // Delete applies to exactly one selected video.
  const deleteTarget = selectedCount === 1 ? selectedVideos[0] ?? null : null;
  const targetProductions = deleteTarget?.productions ?? [];

  const handleDelete = async (deleteFile: boolean) => {
    if (!deleteTarget) return;
    setIsDeleting(true);
    setDeleteError(null);
    try {
      await deleteVideo(deleteTarget.id, deleteFile);
      setDeleteOpen(false);
      onClearSelection();
      onUpdate();
    } catch (e: any) {
      // Includes the backend's 409 "used in a production" message.
      setDeleteError(e.message || 'Failed to delete the video');
    } finally {
      setIsDeleting(false);
    }
  };

  const toggleAddProduction = (id: number) => {
    setAddProductionIds((prev) =>
      prev.includes(id) ? prev.filter((p) => p !== id) : [...prev, id]
    );
  };

  const toggleRemoveProduction = (id: number) => {
    setRemoveProductionIds((prev) =>
      prev.includes(id) ? prev.filter((p) => p !== id) : [...prev, id]
    );
  };

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
      if (addProductionIds.length > 0) {
        request.add_production_ids = addProductionIds;
      }
      if (removeProductionIds.length > 0) {
        request.remove_production_ids = removeProductionIds;
      }

      await bulkUpdateVideos(request);

      // Reset form
      setCategory('');
      setAddTags('');
      setRemoveTags('');
      setAddProductionIds([]);
      setRemoveProductionIds([]);
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
            {deleteTarget && (
              <button
                onClick={() => {
                  setDeleteOpen((o) => !o);
                  setDeleteError(null);
                  setIsOpen(false);
                }}
                className="btn bg-red-600 hover:bg-red-700 text-white"
              >
                🗑 Delete
              </button>
            )}
            <button
              onClick={() => {
                setIsOpen(!isOpen);
                setDeleteOpen(false);
              }}
              className="btn btn-primary"
            >
              Bulk Edit
            </button>
          </div>
        </div>

        {/* Delete confirmation */}
        {deleteOpen && deleteTarget && (
          <div className="mt-4 p-4 bg-gray-50 dark:bg-gray-700 rounded-lg">
            {targetProductions.length > 0 ? (
              <div className="flex items-start gap-3">
                <span className="text-xl">🚫</span>
                <div>
                  <p className="font-medium text-gray-900 dark:text-white">
                    “{deleteTarget.filename}” can’t be deleted
                  </p>
                  <p className="text-sm text-gray-600 dark:text-gray-300 mt-1">
                    It is used in the production{targetProductions.length !== 1 ? 's' : ''}{' '}
                    <span className="font-medium">
                      {targetProductions.map((p) => p.title).join(', ')}
                    </span>
                    . Remove it from the production{targetProductions.length !== 1 ? 's' : ''} first
                    if you really want to delete it.
                  </p>
                </div>
              </div>
            ) : (
              <div>
                <p className="font-medium text-gray-900 dark:text-white">
                  Delete “{deleteTarget.filename}”?
                </p>
                <p className="text-sm text-gray-600 dark:text-gray-300 mt-1 mb-3">
                  Remove it from the library only (the file stays on your drive), or also delete
                  the file from disk. Deleting from disk cannot be undone.
                </p>
                {deleteError && (
                  <p className="text-sm text-red-600 dark:text-red-400 mb-3">{deleteError}</p>
                )}
                <div className="flex flex-wrap items-center gap-2">
                  <button
                    onClick={() => handleDelete(false)}
                    disabled={isDeleting}
                    className="btn btn-secondary text-sm"
                  >
                    {isDeleting ? 'Deleting…' : 'Remove from library'}
                  </button>
                  <button
                    onClick={() => handleDelete(true)}
                    disabled={isDeleting}
                    className="btn bg-red-600 hover:bg-red-700 text-white text-sm"
                  >
                    {isDeleting ? 'Deleting…' : 'Delete file from disk too'}
                  </button>
                  <button
                    onClick={() => setDeleteOpen(false)}
                    disabled={isDeleting}
                    className="btn btn-secondary text-sm"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

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

            {/* Productions */}
            {allProductions.length > 0 && (
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mt-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Add to Productions
                  </label>
                  <div className="max-h-36 overflow-y-auto border border-gray-300 dark:border-gray-600 rounded-lg p-2 space-y-1">
                    {allProductions.map((prod) => (
                      <label
                        key={prod.id}
                        className="flex items-center space-x-2 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-600 p-1 rounded text-sm"
                      >
                        <input
                          type="checkbox"
                          checked={addProductionIds.includes(prod.id)}
                          onChange={() => toggleAddProduction(prod.id)}
                          className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                        />
                        <span className="text-gray-700 dark:text-gray-300 truncate">
                          {prod.title}
                          {prod.platform && (
                            <span className="text-gray-500 dark:text-gray-400"> ({prod.platform})</span>
                          )}
                        </span>
                      </label>
                    ))}
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Remove from Productions
                  </label>
                  <div className="max-h-36 overflow-y-auto border border-gray-300 dark:border-gray-600 rounded-lg p-2 space-y-1">
                    {allProductions.map((prod) => (
                      <label
                        key={prod.id}
                        className="flex items-center space-x-2 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-600 p-1 rounded text-sm"
                      >
                        <input
                          type="checkbox"
                          checked={removeProductionIds.includes(prod.id)}
                          onChange={() => toggleRemoveProduction(prod.id)}
                          className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                        />
                        <span className="text-gray-700 dark:text-gray-300 truncate">
                          {prod.title}
                          {prod.platform && (
                            <span className="text-gray-500 dark:text-gray-400"> ({prod.platform})</span>
                          )}
                        </span>
                      </label>
                    ))}
                  </div>
                </div>
              </div>
            )}

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
