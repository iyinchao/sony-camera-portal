import { useState } from 'react'
import { Camera, Loader2, Search } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Input } from '@/components/ui/input'

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
  const [manualOpen, setManualOpen] = useState(false)
  const trimmed = ip.trim()
  // Manual entry is a fallback: shown when the user opens it, or automatically
  // after auto-discovery fails (an error is present).
  const showManual = manualOpen || !!error

  return (
    <div className="flex min-h-screen flex-1 items-center justify-center p-4">
      <Card className="w-full max-w-[420px] p-7">
        <div className="mb-5 flex items-start gap-3.5">
          <div className="gradient-accent flex h-11 w-11 shrink-0 items-center justify-center rounded-xl text-white shadow-accent">
            <Camera className="h-5 w-5" />
          </div>
          <div>
            <h2 className="text-lg font-semibold leading-tight">Connect to your camera</h2>
            <p className="mt-1 text-sm leading-relaxed text-muted-foreground">
              On the camera:{' '}
              <span className="font-medium text-foreground">
                Send to Smartphone → Select on Smartphone
              </span>
              , then join its Wi-Fi.
            </p>
          </div>
        </div>

        {error && (
          <div className="mb-4 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {error}
          </div>
        )}

        {/* Primary path: auto-discover. */}
        <Button
          variant="primary"
          size="lg"
          className="w-full"
          onClick={onAuto}
          disabled={busy}
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Search className="h-4 w-4" />}
          {busy ? 'Searching for camera…' : 'Auto-discover camera'}
        </Button>

        {/* Fallback path: manual IP, revealed on demand or after a failure. */}
        {showManual ? (
          <div className="mt-4">
            <label className="mb-1.5 block text-xs font-medium uppercase tracking-wider text-muted-foreground">
              Or enter the camera IP
            </label>
            <div className="flex gap-2">
              <Input
                value={ip}
                onChange={(e) => setIp(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && trimmed) onManual(trimmed)
                }}
                placeholder="10.0.0.1"
                inputMode="decimal"
                disabled={busy}
                autoFocus
              />
              <Button
                variant="outline"
                size="lg"
                disabled={busy || !trimmed}
                onClick={() => onManual(trimmed)}
              >
                Connect
              </Button>
            </div>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setManualOpen(true)}
            className="mx-auto mt-3 block text-sm text-muted-foreground transition-colors hover:text-accent"
          >
            Enter IP manually
          </button>
        )}

        {onCancel && (
          <Button variant="ghost" className="mt-3 w-full" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
        )}
      </Card>
    </div>
  )
}
