'use client';

import { useEffect, useRef, useState } from 'react';
import {
  editThumbnailBgUrl,
  fetchEditFrame,
  generateEditTextStyle,
  restyleEditFrame,
  saveEditThumbnail,
} from '@/lib/api';
import type { ThumbnailSpec, ThumbnailTextStyle } from '@/lib/types';
import { DEFAULT_TEXT_STYLE, drawStyledText } from '@/lib/styledText';

const W = 1280;
const H = 720;

type Align = 'left' | 'center' | 'right';

interface ThumbnailEditorProps {
  editId: number;
  duration: number;
  suggestedTexts: string[];
  /** Previously-saved thumbnail state for this run (rehydrated on reopen). */
  saved?: ThumbnailSpec | null;
  /** Topic/title context, used to make AI text styling more relevant. */
  context?: string;
  /** Called with the saved spec after a successful "Save to folder", so the
   *  parent can reflect it immediately (e.g. the "✓ saved" badge). */
  onSaved?: (spec: ThumbnailSpec) => void;
}

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v));
}

/** Render the background still (cover-fit) onto an offscreen canvas and return a
 *  PNG data URL — persisted so an AI restyle / exact frame restores on reopen. */
function renderBackground(img: HTMLImageElement): string | undefined {
  const off = document.createElement('canvas');
  off.width = W;
  off.height = H;
  const ctx = off.getContext('2d');
  if (!ctx) return undefined;
  const scale = Math.max(W / img.width, H / img.height);
  const dw = img.width * scale;
  const dh = img.height * scale;
  ctx.drawImage(img, (W - dw) / 2, (H - dh) / 2, dw, dh);
  return off.toDataURL('image/png');
}

export default function ThumbnailEditor({
  editId,
  duration,
  suggestedTexts,
  saved,
  context,
  onSaved,
}: ThumbnailEditorProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const reqIdRef = useRef(0);
  const draggingRef = useRef(false);
  const [frame, setFrame] = useState<HTMLImageElement | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Text + layout (rehydrated from a saved spec when present).
  const [text, setText] = useState(() => saved?.text ?? suggestedTexts[0] ?? '');
  const [fontSize, setFontSize] = useState(() => saved?.fontSize ?? 110);
  const [uppercase, setUppercase] = useState(() => saved?.uppercase ?? true);
  const [align, setAlign] = useState<Align>(() => saved?.align ?? 'center');
  const [posX, setPosX] = useState(() => saved?.posX ?? 0.5);
  const [posY, setPosY] = useState(() => saved?.posY ?? 0.82);
  const [style, setStyle] = useState<ThumbnailTextStyle>(() =>
    saved?.style ? { ...DEFAULT_TEXT_STYLE, ...saved.style } : DEFAULT_TEXT_STYLE
  );

  const [t, setT] = useState(() => saved?.frameTime ?? clamp(duration / 3, 0, duration));
  const [restyled, setRestyled] = useState(() => saved?.restyled ?? false);

  const [restyling, setRestyling] = useState(false);
  const [restylePrompt, setRestylePrompt] = useState(
    'Enhance this into a punchy YouTube thumbnail: richer contrast and saturation, cinematic color grade, sharper detail, brighter highlights. Keep the subject and composition unchanged. No text.'
  );

  const [styling, setStyling] = useState(false);
  const [textStylePrompt, setTextStylePrompt] = useState('');

  const [saving, setSaving] = useState(false);
  const [savedPath, setSavedPath] = useState<string | null>(null);

  // Latest request wins: while dragging the frame slider, stale frames are
  // discarded.
  const loadFrame = async (loader: () => Promise<Blob>) => {
    const myReq = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const blob = await loader();
      if (myReq !== reqIdRef.current) return;
      const url = URL.createObjectURL(blob);
      const img = new Image();
      img.onload = () => {
        URL.revokeObjectURL(url);
        if (myReq !== reqIdRef.current) return;
        setFrame(img);
        setLoading(false);
      };
      img.onerror = () => {
        URL.revokeObjectURL(url);
        if (myReq !== reqIdRef.current) return;
        setError('Could not load the frame image.');
        setLoading(false);
      };
      img.src = url;
    } catch (e: any) {
      if (myReq !== reqIdRef.current) return;
      setError(e.message || 'Failed to load frame');
      setLoading(false);
    }
  };

  // On open: restore the saved background still (the possibly AI-restyled image),
  // falling back to the original frame at the saved time; otherwise grab the
  // default frame. Runs once.
  const hydratedRef = useRef(false);
  useEffect(() => {
    if (hydratedRef.current) return;
    hydratedRef.current = true;
    if (saved) {
      loadFrame(async () => {
        const res = await fetch(editThumbnailBgUrl(editId, saved.frameTime), {
          cache: 'no-store',
        });
        if (res.ok) return res.blob();
        // No saved background on disk — re-grab the original frame.
        return fetchEditFrame(editId, saved.frameTime);
      });
    } else {
      loadFrame(() => fetchEditFrame(editId, t));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Fetch a fresh frame when the user picks a new time on the slider. Driven
  // explicitly from the slider handler (debounced) rather than a reactive effect
  // on `t`, so a re-mount / React StrictMode double-invoke can never clobber a
  // restored, AI-restyled background with a plain frame.
  const frameDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pickFrameTime = (nextT: number) => {
    setT(nextT);
    setRestyled(false); // picking a new frame drops any AI restyle
    if (frameDebounceRef.current) clearTimeout(frameDebounceRef.current);
    frameDebounceRef.current = setTimeout(() => {
      loadFrame(() => fetchEditFrame(editId, nextT));
    }, 160);
  };
  useEffect(
    () => () => {
      if (frameDebounceRef.current) clearTimeout(frameDebounceRef.current);
    },
    []
  );

  // Redraw whenever the frame or any text/layout/style changes.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, W, H);
    if (frame) {
      const scale = Math.max(W / frame.width, H / frame.height);
      const dw = frame.width * scale;
      const dh = frame.height * scale;
      ctx.drawImage(frame, (W - dw) / 2, (H - dh) / 2, dw, dh);
    } else {
      ctx.fillStyle = '#111827';
      ctx.fillRect(0, 0, W, H);
    }

    drawStyledText(ctx, W, H, { text, fontSize, uppercase, align, posX, posY, style });
  }, [frame, text, fontSize, uppercase, align, posX, posY, style]);

  // Drag anywhere on the canvas to move the text anchor.
  const setPosFromEvent = (clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    setPosX(clamp((clientX - rect.left) / rect.width, 0.02, 0.98));
    setPosY(clamp((clientY - rect.top) / rect.height, 0.05, 0.95));
  };
  const handlePointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    draggingRef.current = true;
    canvasRef.current?.setPointerCapture(e.pointerId);
    setPosFromEvent(e.clientX, e.clientY);
  };
  const handlePointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (draggingRef.current) setPosFromEvent(e.clientX, e.clientY);
  };
  const handlePointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    draggingRef.current = false;
    canvasRef.current?.releasePointerCapture(e.pointerId);
  };

  const handleRestyle = async () => {
    setRestyling(true);
    setError(null);
    try {
      await loadFrame(() => restyleEditFrame(editId, t, restylePrompt));
      setRestyled(true);
    } catch (e: any) {
      setError(e.message || 'AI restyle failed');
    } finally {
      setRestyling(false);
    }
  };

  const handleAiTextStyle = async () => {
    if (!text.trim()) {
      setError('Add some thumbnail text first, then style it.');
      return;
    }
    setStyling(true);
    setError(null);
    try {
      const s = await generateEditTextStyle(editId, {
        text,
        context,
        prompt: textStylePrompt,
      });
      const merged: ThumbnailTextStyle = { ...DEFAULT_TEXT_STYLE, ...s };
      // The band's intended text color becomes the actual fill, so the "Text"
      // color control and the renderer stay consistent (band = color only).
      if (merged.highlight?.textColor) merged.fill = merged.highlight.textColor;
      setStyle(merged);
    } catch (e: any) {
      setError(e.message || 'AI text style failed');
    } finally {
      setStyling(false);
    }
  };

  const handleDownload = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    setError(null);
    try {
      canvas.toBlob((blob) => {
        if (!blob) {
          setError('Could not render the image. Try "Save to folder" instead.');
          return;
        }
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'thumbnail.png';
        // The anchor must be in the DOM for .click() to trigger a download in
        // most WebViews/browsers.
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        setTimeout(() => URL.revokeObjectURL(url), 1000);
      }, 'image/png');
    } catch (e: any) {
      setError(e.message || 'Download failed. Use "Save to folder" instead.');
    }
  };

  const handleSave = async () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    setSaving(true);
    setSavedPath(null);
    setError(null);
    try {
      const spec: ThumbnailSpec = {
        text,
        fontSize,
        uppercase,
        align,
        posX,
        posY,
        frameTime: t,
        restyled,
        style,
      };
      const { path } = await saveEditThumbnail(editId, {
        image: canvas.toDataURL('image/png'),
        background: frame ? renderBackground(frame) : undefined,
        spec,
      });
      setSavedPath(path);
      onSaved?.(spec);
    } catch (e: any) {
      setError(e.message || 'Failed to save thumbnail');
    } finally {
      setSaving(false);
    }
  };

  const alignBtn = (a: Align, label: string) => (
    <button
      type="button"
      onClick={() => setAlign(a)}
      className={`px-2 py-1 text-xs rounded ${
        align === a
          ? 'bg-primary-600 text-white'
          : 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-200'
      }`}
      title={`Align ${a}`}
    >
      {label}
    </button>
  );

  return (
    <div className="space-y-3">
      <canvas
        ref={canvasRef}
        width={W}
        height={H}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        style={{ touchAction: 'none' }}
        className="w-full h-auto rounded border border-gray-200 dark:border-gray-700 bg-black cursor-move"
      />
      <p className="text-xs text-gray-400 dark:text-gray-500 -mt-1">
        Tip: drag on the image to position the text.
      </p>

      {/* Frame picker */}
      <div className="flex items-center gap-3">
        <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap w-28">
          {loading ? 'Loading…' : `Frame at ${t.toFixed(1)}s`}
          {restyled && !loading && <span className="text-purple-500"> ✨</span>}
        </span>
        <input
          type="range"
          min={0}
          max={Math.max(0.1, duration)}
          step={0.1}
          value={t}
          onChange={(e) => pickFrameTime(parseFloat(e.target.value))}
          className="flex-1"
        />
        <button
          type="button"
          onClick={handleRestyle}
          disabled={loading || restyling}
          className="btn btn-secondary text-xs whitespace-nowrap"
          title="Restyle this frame with the configured image model (requires an API key)"
        >
          {restyling ? 'Restyling…' : '✨ AI restyle frame'}
        </button>
      </div>

      {/* AI restyle prompt (optional) */}
      <details className="text-xs text-gray-500 dark:text-gray-400">
        <summary className="cursor-pointer select-none">✨ AI restyle prompt (optional)</summary>
        <textarea
          value={restylePrompt}
          onChange={(e) => setRestylePrompt(e.target.value)}
          rows={2}
          className="input text-sm mt-2"
          placeholder="How to restyle the frame (focus on background/lighting)."
        />
        <p className="mt-1">
          Tip: Gemini often refuses close-up shots of real faces. A wider frame or a
          background/lighting-focused prompt works best.
        </p>
      </details>

      {/* Text */}
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        rows={2}
        className="input text-sm"
        placeholder="Thumbnail text (use line breaks for multiple lines)"
      />
      {suggestedTexts.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {suggestedTexts.map((s, i) => (
            <button
              key={i}
              type="button"
              onClick={() => setText(s)}
              className="badge bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-200 text-xs"
              title="Use this suggestion"
            >
              {s}
            </button>
          ))}
        </div>
      )}

      {/* AI text styling */}
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={handleAiTextStyle}
          disabled={styling}
          className="btn btn-secondary text-xs whitespace-nowrap"
          title="Let AI design an eye-catching text treatment (colors, gradient, shadow, highlight band)"
        >
          {styling ? 'Styling…' : '✨ AI style text'}
        </button>
        <button
          type="button"
          onClick={() => setStyle(DEFAULT_TEXT_STYLE)}
          className="btn btn-secondary text-xs"
          title="Reset to plain white text with a black outline"
        >
          Reset style
        </button>
        <span className="text-xs text-gray-400 dark:text-gray-500">
          Styles the text only — the words stay crisp and editable.
        </span>
      </div>
      <details className="text-xs text-gray-500 dark:text-gray-400">
        <summary className="cursor-pointer select-none">✨ AI text-style direction (optional)</summary>
        <input
          value={textStylePrompt}
          onChange={(e) => setTextStylePrompt(e.target.value)}
          className="input text-sm mt-2"
          placeholder='e.g. "bold red and yellow, aggressive" or "clean minimal white"'
        />
      </details>

      {/* Sliders */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 items-center">
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          Size
          <input type="range" min={40} max={220} step={2} value={fontSize} onChange={(e) => setFontSize(parseInt(e.target.value))} className="flex-1" />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          Outline
          <input
            type="range"
            min={0}
            max={24}
            step={1}
            value={style.outlineWidth}
            onChange={(e) => setStyle((s) => ({ ...s, outlineWidth: parseInt(e.target.value) }))}
            className="flex-1"
          />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          X
          <input type="range" min={0.02} max={0.98} step={0.01} value={posX} onChange={(e) => setPosX(parseFloat(e.target.value))} className="flex-1" />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          Y
          <input type="range" min={0.05} max={0.95} step={0.01} value={posY} onChange={(e) => setPosY(parseFloat(e.target.value))} className="flex-1" />
        </label>
      </div>

      {/* Alignment + colors + caps */}
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <div className="flex items-center gap-1">
          <span className="text-xs text-gray-500 dark:text-gray-400 mr-1">Align</span>
          {alignBtn('left', '⯇')}
          {alignBtn('center', '☰')}
          {alignBtn('right', '⯈')}
        </div>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1">
          Text
          <input
            type="color"
            value={style.fill}
            onChange={(e) => setStyle((s) => ({ ...s, fill: e.target.value, gradient: null }))}
            className="w-7 h-7 p-0 border-0 bg-transparent"
          />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1">
          Edge
          <input
            type="color"
            value={style.outlineColor}
            onChange={(e) => setStyle((s) => ({ ...s, outlineColor: e.target.value }))}
            className="w-7 h-7 p-0 border-0 bg-transparent"
          />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1 cursor-pointer">
          <input type="checkbox" checked={uppercase} onChange={(e) => setUppercase(e.target.checked)} className="rounded" />
          CAPS
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1 cursor-pointer">
          <input
            type="checkbox"
            checked={!!style.highlight}
            onChange={(e) =>
              setStyle((s) => ({
                ...s,
                highlight: e.target.checked
                  ? { color: s.highlight?.color ?? '#e11d2a', textColor: s.fill }
                  : null,
              }))
            }
            className="rounded"
          />
          Band
          {style.highlight && (
            <input
              type="color"
              value={style.highlight.color}
              onChange={(e) =>
                setStyle((s) => ({
                  ...s,
                  highlight: { color: e.target.value, textColor: s.highlight?.textColor ?? s.fill },
                }))
              }
              className="w-7 h-7 p-0 border-0 bg-transparent"
            />
          )}
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1 cursor-pointer">
          <input
            type="checkbox"
            checked={style.shadowBlur > 0}
            onChange={(e) =>
              setStyle((s) => ({
                ...s,
                shadowBlur: e.target.checked ? 18 : 0,
                shadowOffsetY: e.target.checked ? 6 : 0,
                shadowColor: s.shadowColor || '#000000',
              }))
            }
            className="rounded"
          />
          Shadow
        </label>
      </div>

      {error && (
        <div className="p-2 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded text-sm text-red-600 dark:text-red-400">
          {error}
        </div>
      )}
      {savedPath && (
        <p className="text-xs text-green-600 dark:text-green-400 break-all">Saved: {savedPath}</p>
      )}

      <div className="flex items-center gap-2">
        <button onClick={handleSave} disabled={saving} className="btn btn-primary text-sm">
          {saving ? 'Saving…' : 'Save to folder'}
        </button>
        <button onClick={handleDownload} className="btn btn-secondary text-sm">
          Download PNG
        </button>
      </div>
    </div>
  );
}
