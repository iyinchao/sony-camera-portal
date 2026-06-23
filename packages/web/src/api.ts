// Photo mirrors the server's /api/list JSON. thumbUrl/fullUrl point at the
// server's own proxy routes (/api/thumb/:id, /api/photo/:id), never the camera.
export interface Photo {
  id: string
  name: string
  date: string
  thumbUrl: string
  fullUrl: string
}

// ConnState mirrors /api/state and /api/connect responses.
export interface ConnState {
  connected: boolean
  host: string | null
  error: string | null
  photoCount: number
}

export async function getState(): Promise<ConnState> {
  const res = await fetch('/api/state')
  return (await res.json()) as ConnState
}

// connectCamera sets the camera target. Pass a host to connect to a specific IP,
// or omit it to auto-discover. Never throws on a failed connection — inspect the
// returned state's `error` / `connected`.
export async function connectCamera(host?: string): Promise<ConnState> {
  const res = await fetch('/api/connect', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(host ? { host } : {}),
  })
  return (await res.json()) as ConnState
}

// A page of the gallery (mirrors GET /api/list?offset&limit).
export interface PhotoPage {
  photos: Photo[]
  total: number | null
  hasMore: boolean
}

// fetchPage loads one page of photos for the connected camera. On failure it
// surfaces the server's JSON error message when present.
export async function fetchPage(
  offset: number,
  limit: number,
  signal?: AbortSignal,
): Promise<PhotoPage> {
  const res = await fetch(`/api/list?offset=${offset}&limit=${limit}`, { signal })
  if (!res.ok) {
    let message = `Request failed (HTTP ${res.status})`
    try {
      const body = await res.json()
      if (body && typeof body.error === 'string') message = body.error
    } catch {
      // non-JSON body; keep the generic message
    }
    throw new Error(message)
  }
  return (await res.json()) as PhotoPage
}
