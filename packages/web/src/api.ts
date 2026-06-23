// Photo mirrors the server's /api/list JSON. thumbUrl/fullUrl point at the
// server's own proxy routes (/api/thumb/:id, /api/photo/:id), never the camera.
export interface Photo {
  id: string
  name: string
  date: string
  thumbUrl: string
  fullUrl: string
}

// fetchPhotos loads the gallery. On failure it surfaces the server's JSON error
// message (e.g. "connect to the camera Wi-Fi") when present.
export async function fetchPhotos(signal?: AbortSignal): Promise<Photo[]> {
  const res = await fetch('/api/list', { signal })
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
  return (await res.json()) as Photo[]
}
