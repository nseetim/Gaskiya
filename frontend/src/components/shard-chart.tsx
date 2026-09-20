"use client";

import { useId, useState } from "react";

/**
 * Single-series magnitude bar chart (records per Cuckoo shard). Uses the
 * dataviz skill's validated sequential-blue hue rather than the app's
 * neutral UI palette, since chart color has different legibility
 * requirements than interface chrome. See DECISIONS.md is not needed here
 * (no product decision), just the skill's own reference palette.
 */
export function ShardChart({ distribution }: { distribution: number[] }) {
  const [hovered, setHovered] = useState<number | null>(null);
  const gradientId = useId();
  const max = Math.max(1, ...distribution);
  const barWidth = 100 / distribution.length;

  return (
    <div className="viz-root">
      <style>{`
        .viz-root {
          --surface-1: #fcfcfb;
          --text-primary: #0b0b0b;
          --text-secondary: #52514e;
          --series-1: #2a78d6;
          --grid: #e5e4e0;
        }
        @media (prefers-color-scheme: dark) {
          :root:not([data-theme="light"]) .viz-root {
            --surface-1: #1a1a19;
            --text-primary: #ffffff;
            --text-secondary: #c3c2b7;
            --series-1: #3987e5;
            --grid: #33322f;
          }
        }
        :root[data-theme="dark"] .viz-root {
          --surface-1: #1a1a19;
          --text-primary: #ffffff;
          --text-secondary: #c3c2b7;
          --series-1: #3987e5;
          --grid: #33322f;
        }
      `}</style>

      <svg viewBox="0 0 100 56" className="w-full" role="img" aria-label="Records per shard">
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--series-1)" stopOpacity="1" />
            <stop offset="100%" stopColor="var(--series-1)" stopOpacity="0.75" />
          </linearGradient>
        </defs>
        {/* baseline */}
        <line x1="0" y1="48" x2="100" y2="48" stroke="var(--grid)" strokeWidth="0.3" />
        {distribution.map((count, i) => {
          const heightPx = (count / max) * 40;
          const x = i * barWidth + barWidth * 0.15;
          const w = barWidth * 0.7;
          const y = 48 - heightPx;
          return (
            <g key={i} onMouseEnter={() => setHovered(i)} onMouseLeave={() => setHovered(null)}>
              <rect
                x={x}
                y={y}
                width={w}
                height={Math.max(heightPx, 0.5)}
                rx="1"
                fill={`url(#${gradientId})`}
                opacity={hovered === null || hovered === i ? 1 : 0.55}
              />
              <text
                x={x + w / 2}
                y={y - 1.5}
                textAnchor="middle"
                fontSize="4"
                fill="var(--text-primary)"
              >
                {count}
              </text>
              <text
                x={x + w / 2}
                y="52.5"
                textAnchor="middle"
                fontSize="3.5"
                fill="var(--text-secondary)"
              >
                {i}
              </text>
            </g>
          );
        })}
      </svg>
      <p className="mt-1 text-center text-xs" style={{ color: "var(--text-secondary)" }}>
        Shard ID
      </p>

      <details className="mt-2">
        <summary className="cursor-pointer text-xs underline" style={{ color: "var(--text-secondary)" }}>
          View as table
        </summary>
        <table className="mt-2 w-full text-xs">
          <thead>
            <tr className="text-left">
              <th className="pr-4 font-medium">Shard</th>
              <th className="font-medium">Records</th>
            </tr>
          </thead>
          <tbody>
            {distribution.map((count, i) => (
              <tr key={i}>
                <td className="pr-4">{i}</td>
                <td>{count}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </div>
  );
}
