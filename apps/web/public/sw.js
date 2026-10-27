// 最小 Service Worker：接收 ntfy Web Push 并展示通知。
self.addEventListener('push', (event) => {
  let payload = {}
  try {
    payload = event.data ? event.data.json() : {}
  } catch {
    payload = { message: event.data ? event.data.text() : '' }
  }
  event.waitUntil(
    self.registration.showNotification(payload.title || '社团 OA', {
      body: payload.message || '',
      data: { url: payload.click || '/' },
    }),
  )
})

self.addEventListener('notificationclick', (event) => {
  event.notification.close()
  const url = event.notification.data?.url || '/'
  event.waitUntil(
    self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then((clients) => {
      const existing = clients.find((client) => 'focus' in client)
      if (existing) return existing.navigate(url).then(() => existing.focus())
      return self.clients.openWindow(url)
    }),
  )
})
