import { useState } from 'react'

export default function ConnectPanel({
  busy,
  error,
  onAuto,
  onManual,
  onCancel,
}: {
  busy: boolean
  error: string | null
  onAuto: () => void
  onManual: (host: string) => void
  onCancel?: () => void
}) {
  const [ip, setIp] = useState('')
  const trimmed = ip.trim()

  return (
    <div className="centered">
      <div className="connect-card">
        <h2 className="connect-title">
          <span className="dot" /> Connect to your camera
        </h2>

        {busy ? (
          <p className="muted">Searching for the camera…</p>
        ) : (
          <p className="muted">
            On the camera: <b>Send to Smartphone → Select on Smartphone</b>, then join its Wi-Fi.
          </p>
        )}

        {error && <p className="connect-error">{error}</p>}

        <div className="connect-row">
          <input
            className="ip-input"
            value={ip}
            onChange={(e) => setIp(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && trimmed) onManual(trimmed)
            }}
            placeholder="Camera IP (e.g. 10.0.0.1)"
            inputMode="decimal"
            disabled={busy}
          />
          <button className="primary" disabled={busy || !trimmed} onClick={() => onManual(trimmed)}>
            Connect
          </button>
        </div>

        <div className="connect-actions">
          <button onClick={onAuto} disabled={busy}>
            {busy ? 'Searching…' : 'Auto-discover'}
          </button>
          {onCancel && (
            <button onClick={onCancel} disabled={busy}>
              Cancel
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
