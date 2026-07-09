'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import {
  getConfig,
  saveConfig,
  browseFolder,
  isTauri,
  getAiSettings,
  saveAiSettings,
  getIndexStatus,
  reindexSearch,
  getReindexStatus,
} from '@/lib/api';
import type { AiSettings, AiSettingsUpdate, IndexStatus, ReindexProgress } from '@/lib/types';

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

  // AI / LLM settings (desktop only)
  const [showAi] = useState(isTauri());
  const [aiSettings, setAiSettings] = useState<AiSettings | null>(null);
  const [aiForm, setAiForm] = useState<AiSettingsUpdate>({});
  const [aiKeys, setAiKeys] = useState({ gemini: '', openai: '', anthropic: '', elevenlabs: '' });
  const [isSavingAi, setIsSavingAi] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const [aiSuccess, setAiSuccess] = useState<string | null>(null);

  // Semantic search index (desktop only)
  const [indexStatus, setIndexStatus] = useState<IndexStatus | null>(null);
  const [reindexJobId, setReindexJobId] = useState<string | null>(null);
  const [reindexProgress, setReindexProgress] = useState<ReindexProgress | null>(null);
  const [reindexError, setReindexError] = useState<string | null>(null);
  const [transcribeMissing, setTranscribeMissing] = useState(false);
  const [describeVisuals, setDescribeVisuals] = useState(false);

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

    if (isTauri()) {
      try {
        const ai = await getAiSettings();
        setAiSettings(ai);
        setAiForm({
          text_provider: ai.text_provider,
          text_model: ai.text_model,
          transcription_provider: ai.transcription_provider,
          transcription_model: ai.transcription_model,
          image_provider: ai.image_provider,
          image_model: ai.image_model,
          embedding_provider: ai.embedding_provider,
          embedding_model: ai.embedding_model,
          system_prompt: ai.system_prompt,
          edit_prompt: ai.edit_prompt,
          short_edit_prompt: ai.short_edit_prompt,
        });
      } catch {
        // AI settings are optional; ignore load failures.
      }
      try {
        setIndexStatus(await getIndexStatus());
      } catch {
        // Index status is optional; ignore load failures.
      }
    }
  };

  // Poll a running reindex job until it finishes, then refresh coverage.
  useEffect(() => {
    if (!reindexJobId) return;
    let active = true;
    const tick = async () => {
      try {
        const p = await getReindexStatus(reindexJobId);
        if (!active) return;
        setReindexProgress(p);
        if (p.status === 'in_progress') {
          setTimeout(tick, 1500);
        } else {
          setReindexJobId(null);
          if (p.status === 'failed') setReindexError(p.error || 'Reindex failed');
          try {
            setIndexStatus(await getIndexStatus());
          } catch {
            /* ignore */
          }
        }
      } catch {
        if (active) setReindexJobId(null);
      }
    };
    tick();
    return () => {
      active = false;
    };
  }, [reindexJobId]);

  const handleReindex = async () => {
    setReindexError(null);
    setReindexProgress(null);
    try {
      const res = await reindexSearch(transcribeMissing, describeVisuals);
      setReindexJobId(res.job_id);
    } catch (err: any) {
      setReindexError(err.message || 'Failed to start reindex');
    }
  };

  const handleSaveAi = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsSavingAi(true);
    setAiError(null);
    setAiSuccess(null);

    const payload: AiSettingsUpdate = { ...aiForm };
    if (aiKeys.gemini.trim()) payload.gemini_api_key = aiKeys.gemini.trim();
    if (aiKeys.openai.trim()) payload.openai_api_key = aiKeys.openai.trim();
    if (aiKeys.anthropic.trim()) payload.anthropic_api_key = aiKeys.anthropic.trim();
    if (aiKeys.elevenlabs.trim()) payload.elevenlabs_api_key = aiKeys.elevenlabs.trim();

    try {
      await saveAiSettings(payload);
      setAiSuccess('AI settings saved!');
      setTimeout(() => setAiSuccess(null), 3000);
      setAiKeys({ gemini: '', openai: '', anthropic: '', elevenlabs: '' });
      // Refresh key-presence indicators.
      const ai = await getAiSettings();
      setAiSettings(ai);
    } catch (err: any) {
      setAiError(err.message || 'Failed to save AI settings');
    } finally {
      setIsSavingAi(false);
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

        {/* AI / LLM Settings (desktop only) */}
        {showAi && (
          <>
          <form onSubmit={handleSaveAi} className="mt-6 space-y-6">
            <div className="card">
              <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-1">
                AI / LLM
              </h2>
              <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
                Used to transcribe portrait videos and generate thumbnail text and
                Instagram / TikTok / YouTube Short descriptions. Keys are stored locally
                and never displayed after saving.
              </p>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Text provider
                  </label>
                  <select
                    value={aiForm.text_provider || 'gemini'}
                    onChange={(e) => setAiForm({ ...aiForm, text_provider: e.target.value })}
                    className="input"
                  >
                    <option value="gemini">Google Gemini</option>
                    <option value="openai">OpenAI</option>
                    <option value="anthropic">Anthropic Claude</option>
                  </select>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Text model
                  </label>
                  <input
                    type="text"
                    value={aiForm.text_model || ''}
                    onChange={(e) => setAiForm({ ...aiForm, text_model: e.target.value })}
                    className="input"
                    placeholder="e.g. gemini-2.0-flash, gpt-4o, claude-sonnet-4-6"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Transcription provider
                  </label>
                  <select
                    value={aiForm.transcription_provider || 'elevenlabs'}
                    onChange={(e) => {
                      const provider = e.target.value;
                      // Model ids are provider-specific, so switching providers
                      // re-seeds the model with that provider's default.
                      const defaultModel =
                        provider === 'elevenlabs' ? 'scribe_v1' :
                        provider === 'openai' ? 'whisper-1' : '';
                      setAiForm({
                        ...aiForm,
                        transcription_provider: provider,
                        transcription_model: defaultModel || aiForm.transcription_model,
                      });
                    }}
                    className="input"
                  >
                    <option value="elevenlabs">ElevenLabs (Scribe)</option>
                    <option value="openai">OpenAI (Whisper)</option>
                    {/* Legacy only: Gemini returns no word timestamps, so captions,
                        tighten and music ducking degrade. Kept while the SAVED
                        setting is still gemini (so a user can switch back before
                        saving), or while it's the current form value. */}
                    {(aiSettings?.transcription_provider === 'gemini' ||
                      aiForm.transcription_provider === 'gemini') && (
                      <option value="gemini">Google Gemini (legacy — no word timestamps)</option>
                    )}
                  </select>
                  {aiForm.transcription_provider === 'gemini' && (
                    <p className="mt-1 text-xs text-amber-600 dark:text-amber-400">
                      Gemini transcription has no word timestamps — captions, silence
                      tightening and music ducking won&apos;t work. Switch to ElevenLabs
                      or Whisper.
                    </p>
                  )}
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Transcription model
                  </label>
                  <input
                    type="text"
                    value={aiForm.transcription_model || ''}
                    onChange={(e) =>
                      setAiForm({ ...aiForm, transcription_model: e.target.value })
                    }
                    className="input"
                    placeholder="e.g. scribe_v1, whisper-1"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Image (thumbnail) provider
                  </label>
                  <select
                    value={aiForm.image_provider || 'gemini'}
                    onChange={(e) => setAiForm({ ...aiForm, image_provider: e.target.value })}
                    className="input"
                  >
                    <option value="gemini">Google Gemini</option>
                    <option value="openai">OpenAI (GPT Image)</option>
                  </select>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Image (thumbnail) model
                  </label>
                  <input
                    type="text"
                    value={aiForm.image_model || ''}
                    onChange={(e) => setAiForm({ ...aiForm, image_model: e.target.value })}
                    className="input"
                    placeholder="e.g. gemini-2.5-flash-image, gpt-image-1, gpt-image-2"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Embedding (search) provider
                  </label>
                  <select
                    value={aiForm.embedding_provider || 'openai'}
                    onChange={(e) => {
                      const provider = e.target.value;
                      // Model ids are provider-specific — re-seed the default.
                      const defaultModel =
                        provider === 'openai' ? 'text-embedding-3-small' :
                        provider === 'gemini' ? 'text-embedding-004' : '';
                      setAiForm({
                        ...aiForm,
                        embedding_provider: provider,
                        embedding_model: defaultModel || aiForm.embedding_model,
                      });
                    }}
                    className="input"
                  >
                    <option value="openai">OpenAI</option>
                    <option value="gemini">Google Gemini</option>
                  </select>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    Embedding (search) model
                  </label>
                  <input
                    type="text"
                    value={aiForm.embedding_model || ''}
                    onChange={(e) => setAiForm({ ...aiForm, embedding_model: e.target.value })}
                    className="input"
                    placeholder="e.g. text-embedding-3-small, text-embedding-004"
                  />
                </div>
              </div>

              <p className="text-xs text-gray-500 dark:text-gray-400 mt-3">
                The image provider/model powers the ✨ AI restyle in the thumbnail builder. OpenAI&apos;s
                GPT Image models render text best and edit photos more permissively; Gemini is cheaper.
                Editing close-up shots of real, identifiable faces may be refused by either provider.
              </p>

              <p className="text-xs text-gray-500 dark:text-gray-400 mt-3">
                ElevenLabs (Scribe) and OpenAI (Whisper) return word-level timestamps, which the
                video-edit pipeline uses to choose precise cut points. Anthropic Claude does not
                transcribe audio.
              </p>

              <div className="mt-5 space-y-4">
                {([
                  { key: 'elevenlabs', label: 'ElevenLabs API key', set: aiSettings?.elevenlabs_api_key_set },
                  { key: 'gemini', label: 'Gemini API key', set: aiSettings?.gemini_api_key_set },
                  { key: 'openai', label: 'OpenAI API key', set: aiSettings?.openai_api_key_set },
                  { key: 'anthropic', label: 'Anthropic API key', set: aiSettings?.anthropic_api_key_set },
                ] as const).map(({ key, label, set }) => (
                  <div key={key}>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {label}
                      {set && (
                        <span className="ml-2 text-xs text-green-600 dark:text-green-400">
                          ✓ saved
                        </span>
                      )}
                    </label>
                    <input
                      type="password"
                      autoComplete="off"
                      value={aiKeys[key]}
                      onChange={(e) => setAiKeys({ ...aiKeys, [key]: e.target.value })}
                      className="input"
                      placeholder={set ? 'Leave blank to keep current key' : 'Paste API key'}
                    />
                  </div>
                ))}
              </div>

              {/* Editable system prompt */}
              <div className="mt-6">
                <div className="flex items-center justify-between mb-1">
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Content generation prompt
                  </label>
                  <button
                    type="button"
                    onClick={() =>
                      aiSettings &&
                      setAiForm({ ...aiForm, system_prompt: aiSettings.default_system_prompt })
                    }
                    disabled={
                      !aiSettings || aiForm.system_prompt === aiSettings.default_system_prompt
                    }
                    className="text-xs text-primary-600 dark:text-primary-400 hover:underline disabled:opacity-40 disabled:no-underline"
                  >
                    Reset to default
                  </button>
                </div>
                <p className="text-xs text-gray-500 dark:text-gray-400 mb-2">
                  The instructions sent to the text model. Use the token{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">{'{transcript}'}</code>{' '}
                  where the video transcript should be inserted (it&apos;s appended automatically if
                  you remove it). The model must return JSON with the keys{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">thumbnail_texts</code>,{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">instagram_description</code>,{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">tiktok_description</code>,{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">youtube_short_title</code>,{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">youtube_short_description</code>,{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">youtube_short_tags</code>, and{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">hashtags</code>.
                </p>
                <textarea
                  value={aiForm.system_prompt ?? ''}
                  onChange={(e) => setAiForm({ ...aiForm, system_prompt: e.target.value })}
                  rows={14}
                  spellCheck={false}
                  className="input font-mono text-xs leading-relaxed"
                  placeholder="Loading prompt…"
                />
              </div>

              {/* Editable video-edit pipeline prompt */}
              <div className="mt-6">
                <div className="flex items-center justify-between mb-1">
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Video-edit pipeline prompt
                  </label>
                  <button
                    type="button"
                    onClick={() =>
                      aiSettings &&
                      setAiForm({ ...aiForm, edit_prompt: aiSettings.default_edit_prompt })
                    }
                    disabled={
                      !aiSettings || aiForm.edit_prompt === aiSettings.default_edit_prompt
                    }
                    className="text-xs text-primary-600 dark:text-primary-400 hover:underline disabled:opacity-40 disabled:no-underline"
                  >
                    Reset to default
                  </button>
                </div>
                <p className="text-xs text-gray-500 dark:text-gray-400 mb-2">
                  Drives the “Edit &amp; Create Video” pipeline that stitches the best raw takes
                  into a final clip. Use{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">{'{script}'}</code> and{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">{'{transcripts}'}</code>{' '}
                  where the script and the per-take timestamped transcripts should be inserted
                  (each is appended automatically if removed). The model must return JSON with a{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">scenes</code> array of{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">clips</code>, each with{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">video_id</code>,{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">start</code>, and{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">end</code> (seconds).
                </p>
                <textarea
                  value={aiForm.edit_prompt ?? ''}
                  onChange={(e) => setAiForm({ ...aiForm, edit_prompt: e.target.value })}
                  rows={14}
                  spellCheck={false}
                  className="input font-mono text-xs leading-relaxed"
                  placeholder="Loading prompt…"
                />
              </div>

              {/* Editable short-form (script-less) cleanup prompt */}
              <div className="mt-6">
                <div className="flex items-center justify-between mb-1">
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                    Short-form cleanup prompt
                  </label>
                  <button
                    type="button"
                    onClick={() =>
                      aiSettings &&
                      setAiForm({ ...aiForm, short_edit_prompt: aiSettings.default_short_edit_prompt })
                    }
                    disabled={
                      !aiSettings || aiForm.short_edit_prompt === aiSettings.default_short_edit_prompt
                    }
                    className="text-xs text-primary-600 dark:text-primary-400 hover:underline disabled:opacity-40 disabled:no-underline"
                  >
                    Reset to default
                  </button>
                </div>
                <p className="text-xs text-gray-500 dark:text-gray-400 mb-2">
                  Used when an edit is started <em>without</em> a script (typical for short-form,
                  single-take videos): the transcript is the script and the model plans a cleanup
                  cut — false starts, repeated content, filler, dead air. Use{' '}
                  <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">{'{transcripts}'}</code>{' '}
                  where the timestamped transcripts should be inserted. Same JSON output contract
                  as the pipeline prompt above.
                </p>
                <textarea
                  value={aiForm.short_edit_prompt ?? ''}
                  onChange={(e) => setAiForm({ ...aiForm, short_edit_prompt: e.target.value })}
                  rows={14}
                  spellCheck={false}
                  className="input font-mono text-xs leading-relaxed"
                  placeholder="Loading prompt…"
                />
              </div>

              {aiError && (
                <div className="mt-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                  <p className="text-sm text-red-600 dark:text-red-400">{aiError}</p>
                </div>
              )}
              {aiSuccess && (
                <div className="mt-4 p-3 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg">
                  <p className="text-sm text-green-600 dark:text-green-400">{aiSuccess}</p>
                </div>
              )}

              <div className="mt-5 flex justify-end">
                <button type="submit" disabled={isSavingAi} className="btn btn-primary">
                  {isSavingAi ? 'Saving...' : 'Save AI Settings'}
                </button>
              </div>
            </div>
          </form>

          {/* Semantic search index */}
          <div className="card mt-6">
            <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-1">
              Semantic search index
            </h2>
            <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
              Powers the ✨ Semantic toggle on the videos page — searching by meaning
              (&ldquo;me talking about parenting&rdquo;) instead of exact keywords. Build the
              index after scanning or generating transcripts; rebuilds are incremental
              (only changed videos are re-embedded). Uses your{' '}
              <strong>{aiForm.embedding_provider || 'openai'}</strong> API key.
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400 mb-4">
              Only videos with <strong>describable text</strong> are indexed — a transcript,
              tags, notes, a category, or a descriptive filename. Raw clips whose only text is
              a camera filename (e.g. <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">GX011916.MP4</code>)
              are skipped, so <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">indexed</code> is
              usually lower than the total. To make more videos searchable, add tags/notes or
              generate transcripts. (Semantic search is text-only — it can&apos;t match purely
              visual content that nothing describes.)
            </p>

            {indexStatus && (
              <div className="text-sm text-gray-700 dark:text-gray-300 space-y-1 mb-4">
                <p>
                  Videos indexed:{' '}
                  <strong>
                    {indexStatus.videos_indexed} / {indexStatus.videos_total}
                  </strong>
                </p>
                <p>
                  Productions indexed:{' '}
                  <strong>
                    {indexStatus.productions_indexed} / {indexStatus.productions_total}
                  </strong>
                </p>
                <p className="text-xs text-gray-500 dark:text-gray-400">
                  Model: <code className="px-1 bg-gray-100 dark:bg-gray-700 rounded">{indexStatus.model}</code>
                </p>
              </div>
            )}

            {reindexProgress && reindexProgress.status === 'in_progress' && (() => {
              const p = reindexProgress;
              let done = p.processed;
              let tot = p.total;
              let label = `${p.stage} — `;
              if (p.stage === 'transcribing') {
                done = p.transcribed + p.transcribe_failed;
                tot = p.transcribe_total;
                label = 'Transcribing videos without a transcript… ';
              } else if (p.stage === 'describing') {
                done = p.described + p.describe_failed;
                tot = p.describe_total;
                label = 'Describing visuals from thumbnails… ';
              }
              const pct = tot > 0 ? Math.round((done / tot) * 100) : 0;
              return (
                <div className="mb-4">
                  <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2 overflow-hidden">
                    <div className="bg-primary-600 h-2 transition-all" style={{ width: `${pct}%` }} />
                  </div>
                  <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                    {label}
                    {done} / {tot}
                  </p>
                </div>
              );
            })()}

            {reindexProgress && reindexProgress.status === 'completed' && (
              <p className="mb-4 text-sm text-green-600 dark:text-green-400">
                Index built — {reindexProgress.videos_indexed} videos and{' '}
                {reindexProgress.productions_indexed} productions embedded (
                {reindexProgress.videos_skipped + reindexProgress.productions_skipped} unchanged)
                {reindexProgress.transcribe_total > 0
                  ? `. Transcribed ${reindexProgress.transcribed} clip${reindexProgress.transcribed === 1 ? '' : 's'}${
                      reindexProgress.transcribe_failed > 0 ? ` (${reindexProgress.transcribe_failed} failed)` : ''
                    }`
                  : ''}
                {reindexProgress.describe_total > 0
                  ? `. Described ${reindexProgress.described} clip${reindexProgress.described === 1 ? '' : 's'}${
                      reindexProgress.describe_failed > 0 ? ` (${reindexProgress.describe_failed} failed)` : ''
                    }`
                  : ''}
                .
              </p>
            )}

            {reindexError && (
              <div className="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                <p className="text-sm text-red-600 dark:text-red-400">{reindexError}</p>
              </div>
            )}

            <label className="flex items-start gap-2 mb-3 cursor-pointer">
              <input
                type="checkbox"
                checked={transcribeMissing}
                onChange={(e) => setTranscribeMissing(e.target.checked)}
                disabled={!!reindexJobId}
                className="mt-0.5 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
              />
              <span className="text-xs text-gray-600 dark:text-gray-400">
                <strong>Transcribe videos with no transcript first.</strong> Much slower and uses
                your <strong>{aiForm.transcription_provider || 'elevenlabs'}</strong> transcription
                API, but makes talking videos searchable by what&apos;s said. Silent / action clips
                (no speech) are skipped. Already-transcribed videos are never redone.
              </span>
            </label>

            <label className="flex items-start gap-2 mb-3 cursor-pointer">
              <input
                type="checkbox"
                checked={describeVisuals}
                onChange={(e) => setDescribeVisuals(e.target.checked)}
                disabled={!!reindexJobId}
                className="mt-0.5 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
              />
              <span className="text-xs text-gray-600 dark:text-gray-400">
                <strong>Describe visuals for videos with no description first.</strong> Analyzes a
                few thumbnails of each clip with your <strong>{aiForm.text_provider || 'gemini'}</strong>{' '}
                vision LLM and stores a short caption + tags — so <em>visual</em> content
                (&ldquo;running in the snow&rdquo;) becomes searchable even with no transcript.
                Slower and costs LLM API money per clip; already-described videos are never redone.
              </span>
            </label>

            <button
              type="button"
              onClick={handleReindex}
              disabled={!!reindexJobId}
              className="btn btn-primary"
            >
              {reindexJobId
                ? 'Building…'
                : transcribeMissing || describeVisuals
                ? 'Analyze + rebuild index'
                : 'Rebuild index'}
            </button>
          </div>
          </>
        )}

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
