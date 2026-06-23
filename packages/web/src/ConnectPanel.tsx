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
  const trimmed = ip.trim()

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

        <label className="mb-1.5 block text-xs font-medium uppercase tracking-wider text-muted-foreground">
          Camera IP address
        </label>
        <div className="flex flex-col gap-2.5">
          <Input
            value={ip}
            onChange={(e) => setIp(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && trimmed) onManual(trimmed)
            }}
            placeholder="10.0.0.1"
            inputMode="decimal"
            disabled={busy}
          />
          <Button
            variant="primary"
            size="lg"
            className="w-full"
            disabled={busy || !trimmed}
            onClick={() => onManual(trimmed)}
          >
            {busy && <Loader2 className="h-4 w-4 animate-spin" />}
            Connect
          </Button>
        </div>

        <div className="my-4 flex items-center gap-3 text-xs text-muted-foreground">
          <span className="h-px flex-1 bg-border" />
          or
          <span className="h-px flex-1 bg-border" />
        </div>

        <Button
          variant="outline"
          size="lg"
          className="w-full"
          onClick={onAuto}
          disabled={busy}
        >
          <Search className="h-4 w-4" />
          {busy ? 'Searching…' : 'Auto-discover camera'}
        </Button>

        {onCancel && (
          <Button variant="ghost" className="mt-2 w-full" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
        )}
      </Card>
    </div>
  )
}
