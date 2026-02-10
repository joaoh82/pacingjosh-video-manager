'use client';

import { useState, useEffect } from 'react';
import { Production } from '@/lib/types';
import {
  getProductions,
  createProduction,
  updateProduction,
  deleteProduction,
} from '@/lib/api';

const PLATFORM_OPTIONS = ['YouTube', 'TikTok', 'Instagram', 'Facebook', 'Twitter/X', 'Vimeo', 'Other'];

interface ProductionManagerProps {
  isOpen: boolean;
  onClose: () => void;
  onUpdate?: () => void;
}

export default function ProductionManager({
  isOpen,
  onClose,
  onUpdate,
}: ProductionManagerProps) {
  const [productions, setProductions] = useState<Production[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [searchTerm, setSearchTerm] = useState('');
  const [editingId, setEditingId] = useState<number | null>(null);
  const [formTitle, setFormTitle] = useState('');
  const [formPlatform, setFormPlatform] = useState('');
  const [formLink, setFormLink] = useState('');
  const [formPublished, setFormPublished] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
      loadProductions();
    } else {
      document.body.style.overflow = 'unset';
    }
    return () => {
      document.body.style.overflow = 'unset';
    };
  }, [isOpen]);

  if (!isOpen) return null;

  async function loadProductions() {
    setIsLoading(true);
    try {
      const data = await getProductions();
      setProductions(data);
    } catch {
      setError('Failed to load productions');
    } finally {
      setIsLoading(false);
    }
  }

  function resetForm() {
    setEditingId(null);
    setFormTitle('');
    setFormPlatform('');
    setFormLink('');
    setFormPublished(false);
    setError(null);
  }

  function startEdit(prod: Production) {
    setEditingId(prod.id);
    setFormTitle(prod.title);
    setFormPlatform(prod.platform || '');
    setFormLink(prod.link || '');
    setFormPublished(prod.is_published);
    setError(null);
  }

  async function handleSave() {
    if (!formTitle.trim()) {
      setError('Title is required');
      return;
    }

    setIsSaving(true);
    setError(null);
    try {
      const payload = {
        title: formTitle.trim(),
        platform: formPlatform.trim() || null,
        link: formLink.trim() || null,
        is_published: formPublished,
      };
      if (editingId !== null) {
        await updateProduction(editingId, payload);
      } else {
        await createProduction(payload);
      }
      resetForm();
      await loadProductions();
      onUpdate?.();
    } catch (err: any) {
      setError(err.message || 'Failed to save production');
    } finally {
      setIsSaving(false);
    }
  }

  async function handleDelete(id: number) {
    if (!confirm('Delete this production? It will be unlinked from all videos.')) return;

    try {
      await deleteProduction(id);
      if (editingId === id) resetForm();
      await loadProductions();
      onUpdate?.();
    } catch (err: any) {
      setError(err.message || 'Failed to delete production');
    }
  }

  const filtered = productions.filter(
    (p) =>
      p.title.toLowerCase().includes(searchTerm.toLowerCase()) ||
      (p.platform || '').toLowerCase().includes(searchTerm.toLowerCase()) ||
      (p.link || '').toLowerCase().includes(searchTerm.toLowerCase())
  );

  return (
    <div className="fixed inset-0 z-50 overflow-y-auto">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black bg-opacity-50 transition-opacity"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="flex min-h-full items-center justify-center p-4">
        <div className="relative w-full max-w-2xl bg-white dark:bg-gray-800 rounded-lg shadow-xl overflow-hidden">
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b dark:border-gray-700">
            <h2 className="text-xl font-semibold text-gray-900 dark:text-white">
              Productions
            </h2>
            <button
              onClick={onClose}
              className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
            >
              <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          {/* Content */}
          <div className="p-4 space-y-4 max-h-[70vh] overflow-y-auto">
            {/* Error */}
            {error && (
              <div className="p-3 bg-red-50 dark:bg-red-900/30 text-red-700 dark:text-red-300 rounded-lg text-sm">
                {error}
              </div>
            )}

            {/* Add / Edit Form */}
            <div className="space-y-3 p-4 bg-gray-50 dark:bg-gray-700/50 rounded-lg">
              <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">
                {editingId !== null ? 'Edit Production' : 'Add Production'}
              </h3>
              <div className="grid grid-cols-2 gap-2">
                <input
                  type="text"
                  value={formTitle}
                  onChange={(e) => setFormTitle(e.target.value)}
                  className="input"
                  placeholder="Production title *"
                />
                <select
                  value={formPlatform}
                  onChange={(e) => setFormPlatform(e.target.value)}
                  className="input"
                >
                  <option value="">Platform (optional)</option>
                  {PLATFORM_OPTIONS.map((p) => (
                    <option key={p} value={p}>{p}</option>
                  ))}
                </select>
              </div>
              <input
                type="url"
                value={formLink}
                onChange={(e) => setFormLink(e.target.value)}
                className="input"
                placeholder="Link (optional) - https://..."
              />
              <label className="flex items-center space-x-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={formPublished}
                  onChange={(e) => setFormPublished(e.target.checked)}
                  className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                />
                <span className="text-sm text-gray-700 dark:text-gray-300">Published</span>
              </label>
              <div className="flex gap-2">
                <button
                  onClick={handleSave}
                  disabled={isSaving}
                  className="btn btn-primary text-sm"
                >
                  {isSaving ? 'Saving...' : editingId !== null ? 'Update' : 'Add'}
                </button>
                {editingId !== null && (
                  <button
                    onClick={resetForm}
                    className="btn btn-secondary text-sm"
                  >
                    Cancel
                  </button>
                )}
              </div>
            </div>

            {/* Search */}
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="input"
              placeholder="Search productions..."
            />

            {/* List */}
            {isLoading ? (
              <div className="text-center py-8">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600 mx-auto" />
              </div>
            ) : filtered.length === 0 ? (
              <p className="text-sm text-gray-500 dark:text-gray-400 text-center py-4 italic">
                {productions.length === 0
                  ? 'No productions yet. Add one above.'
                  : 'No productions match your search.'}
              </p>
            ) : (
              <div className="space-y-2">
                {filtered.map((prod) => (
                  <div
                    key={prod.id}
                    className="flex items-center justify-between p-3 bg-white dark:bg-gray-800 border dark:border-gray-700 rounded-lg"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-gray-900 dark:text-white truncate">
                          {prod.title}
                        </span>
                        {prod.platform && (
                          <span className="text-xs px-1.5 py-0.5 rounded bg-gray-200 dark:bg-gray-600 text-gray-600 dark:text-gray-300 flex-shrink-0">
                            {prod.platform}
                          </span>
                        )}
                        {prod.is_published ? (
                          <span className="text-xs px-1.5 py-0.5 rounded bg-green-100 dark:bg-green-900/40 text-green-700 dark:text-green-400 flex-shrink-0">
                            Published
                          </span>
                        ) : (
                          <span className="text-xs px-1.5 py-0.5 rounded bg-yellow-100 dark:bg-yellow-900/40 text-yellow-700 dark:text-yellow-400 flex-shrink-0">
                            Draft
                          </span>
                        )}
                      </div>
                      {prod.link && (
                        <a
                          href={prod.link}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-sm text-primary-600 dark:text-primary-400 hover:underline truncate block"
                        >
                          {prod.link}
                        </a>
                      )}
                      <span className="text-xs text-gray-500 dark:text-gray-400">
                        {prod.video_count ?? 0} video{(prod.video_count ?? 0) !== 1 ? 's' : ''}
                      </span>
                    </div>
                    <div className="flex items-center gap-2 ml-3 flex-shrink-0">
                      <button
                        onClick={() => startEdit(prod)}
                        className="text-gray-500 hover:text-primary-600 dark:text-gray-400 dark:hover:text-primary-400"
                        title="Edit"
                      >
                        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
                        </svg>
                      </button>
                      <button
                        onClick={() => handleDelete(prod.id)}
                        className="text-gray-500 hover:text-red-600 dark:text-gray-400 dark:hover:text-red-400"
                        title="Delete"
                      >
                        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
