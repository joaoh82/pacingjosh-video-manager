'use client';

import { useEffect, useRef, useState } from 'react';
import { fetchEditFrame, restyleEditFrame, saveEditThumbnail } from '@/lib/api';

const W = 1280;
const H = 720;

interface ThumbnailEditorProps {
  editId: number;
  duration: number;
  suggestedTexts: string[];
}

function wrapLines(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string[] {
  const out: string[] = [];
  for (const para of text.split('\n')) {
    const words = para.split(/\s+/).filter(Boolean);
    if (words.length === 0) {
      out.push('');
      continue;
    }
    let line = words[0];
    for (let i = 1; i < words.length; i++) {
      const test = `${line} ${words[i]}`;
      if (ctx.measureText(test).width > maxWidth) {
        out.push(line);
        line = words[i];
      } else {
        line = test;
      }
    }
    out.push(line);
  }
  return out;
}

export default function ThumbnailEditor({ editId, duration, suggestedTexts }: ThumbnailEditorProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [frame, setFrame] = useState<HTMLImageElement | null>(null);
  const [t, setT] = useState(Math.max(0, Math.min(duration, duration / 3)));
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [text, setText] = useState(suggestedTexts[0] || '');
  const [fontSize, setFontSize] = useState(110);
  const [color, setColor] = useState('#ffffff');
  const [outlineColor, setOutlineColor] = useState('#000000');
  const [outline, setOutline] = useState(10);
  const [posY, setPosY] = useState(0.82);
  const [uppercase, setUppercase] = useState(true);

  const [restyling, setRestyling] = useState(false);
  const [restylePrompt, setRestylePrompt] = useState(
    'Enhance this into a punchy YouTube thumbnail: richer contrast and saturation, cinematic color grade, sharper detail, brighter highlights. Keep the subject and composition unchanged. No text.'
  );
  const [saving, setSaving] = useState(false);
  const [savedPath, setSavedPath] = useState<string | null>(null);

  const loadFrame = async (loader: () => Promise<Blob>) => {
    setLoading(true);
    setError(null);
    try {
      const blob = await loader();
      const url = URL.createObjectURL(blob);
      const img = new Image();
      img.onload = () => {
        setFrame(img);
        URL.revokeObjectURL(url);
        setLoading(false);
      };
      img.onerror = () => {
        URL.revokeObjectURL(url);
        setError('Could not load the frame image.');
        setLoading(false);
      };
      img.src = url;
    } catch (e: any) {
      setError(e.message || 'Failed to load frame');
      setLoading(false);
    }
  };

  // Load an initial frame once.
  useEffect(() => {
    loadFrame(() => fetchEditFrame(editId, t));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editId]);

  // Redraw whenever the frame or any styling changes.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, W, H);
    if (frame) {
      // cover
      const scale = Math.max(W / frame.width, H / frame.height);
      const dw = frame.width * scale;
      const dh = frame.height * scale;
      ctx.drawImage(frame, (W - dw) / 2, (H - dh) / 2, dw, dh);
    } else {
      ctx.fillStyle = '#111827';
      ctx.fillRect(0, 0, W, H);
    }

    const value = uppercase ? text.toUpperCase() : text;
    if (value.trim()) {
      ctx.font = `900 ${fontSize}px Arial, "Arial Black", sans-serif`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.lineJoin = 'round';
      const lines = wrapLines(ctx, value, W * 0.9);
      const lineH = fontSize * 1.1;
      const total = lines.length * lineH;
      let y = posY * H - total / 2 + lineH / 2;
      for (const line of lines) {
        if (outline > 0) {
          ctx.lineWidth = outline * 2;
          ctx.strokeStyle = outlineColor;
          ctx.strokeText(line, W / 2, y);
        }
        ctx.fillStyle = color;
        ctx.fillText(line, W / 2, y);
        y += lineH;
      }
    }
  }, [frame, text, fontSize, color, outline, outlineColor, posY, uppercase]);

  const handleRestyle = async () => {
    setRestyling(true);
    setError(null);
    try {
      await loadFrame(() => restyleEditFrame(editId, t, restylePrompt));
    } catch (e: any) {
      setError(e.message || 'AI restyle failed');
    } finally {
      setRestyling(false);
    }
  };

  const handleDownload = () => {
    canvasRef.current?.toBlob((blob) => {
      if (!blob) return;
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'thumbnail.png';
      a.click();
      URL.revokeObjectURL(url);
    }, 'image/png');
  };

  const handleSave = async () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    setSaving(true);
    setSavedPath(null);
    setError(null);
    try {
      const { path } = await saveEditThumbnail(editId, canvas.toDataURL('image/png'));
      setSavedPath(path);
    } catch (e: any) {
      setError(e.message || 'Failed to save thumbnail');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-3">
      <canvas
        ref={canvasRef}
        width={W}
        height={H}
        className="w-full h-auto rounded border border-gray-200 dark:border-gray-700 bg-black"
      />

      {/* Frame picker */}
      <div className="flex items-center gap-3">
        <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap w-28">
          Frame at {t.toFixed(1)}s
        </span>
        <input
          type="range"
          min={0}
          max={Math.max(0.1, duration)}
          step={0.1}
          value={t}
          onChange={(e) => setT(parseFloat(e.target.value))}
          className="flex-1"
        />
        <button
          type="button"
          onClick={() => loadFrame(() => fetchEditFrame(editId, t))}
          disabled={loading || restyling}
          className="btn btn-secondary text-xs whitespace-nowrap"
        >
          {loading ? 'Loading…' : 'Grab frame'}
        </button>
        <button
          type="button"
          onClick={handleRestyle}
          disabled={loading || restyling}
          className="btn btn-secondary text-xs whitespace-nowrap"
          title="Restyle this frame with Gemini's image model (requires a Gemini API key)"
        >
          {restyling ? 'Restyling…' : '✨ AI restyle'}
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

      {/* Style controls */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 items-center">
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          Size
          <input type="range" min={40} max={220} step={2} value={fontSize} onChange={(e) => setFontSize(parseInt(e.target.value))} className="flex-1" />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          Outline
          <input type="range" min={0} max={24} step={1} value={outline} onChange={(e) => setOutline(parseInt(e.target.value))} className="flex-1" />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          Position
          <input type="range" min={0.1} max={0.95} step={0.01} value={posY} onChange={(e) => setPosY(parseFloat(e.target.value))} className="flex-1" />
        </label>
        <div className="flex items-center gap-3">
          <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1">
            Text
            <input type="color" value={color} onChange={(e) => setColor(e.target.value)} className="w-7 h-7 p-0 border-0 bg-transparent" />
          </label>
          <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1">
            Edge
            <input type="color" value={outlineColor} onChange={(e) => setOutlineColor(e.target.value)} className="w-7 h-7 p-0 border-0 bg-transparent" />
          </label>
          <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1 cursor-pointer">
            <input type="checkbox" checked={uppercase} onChange={(e) => setUppercase(e.target.checked)} className="rounded" />
            CAPS
          </label>
        </div>
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
