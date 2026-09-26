import { ref, computed, watch } from 'vue'
import { setRootEnabled } from '@/bridge/app'
import type { AppConfig, ClashApiProfile } from '@/types'

const STORAGE_KEY = 'singboard-config'

export const SERVICE_HOME = '/data/adb/singboard'

function createClashApiId(): string {
  return `api_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`
}

function createClashApiProfile(name: string, url: string, secret: string): ClashApiProfile {
  return {
    id: createClashApiId(),
    name,
    url,
    secret,
  }
}

function normalizeConfig(raw: any): AppConfig {
  const normalizePath = (value: unknown): string => (typeof value === 'string' ? value.trim() : '')

  const savedProfiles = Array.isArray(raw?.clashApis)
    ? raw.clashApis
      .filter((item: any) => item && typeof item === 'object')
      .map((item: any): ClashApiProfile => ({
        id: typeof item.id === 'string' && item.id ? item.id : createClashApiId(),
        name: typeof item.name === 'string' && item.name.trim() ? item.name.trim() : 'API',
        url: typeof item.url === 'string' && item.url.trim() ? item.url.trim() : 'http://127.0.0.1:9090',
        secret: typeof item.secret === 'string' ? item.secret : '',
      }))
    : []

  const legacyUrl = typeof raw?.clashApiUrl === 'string' && raw.clashApiUrl.trim()
    ? raw.clashApiUrl.trim()
    : 'http://127.0.0.1:9090'
  const legacySecret = typeof raw?.clashApiSecret === 'string' ? raw.clashApiSecret : ''

  const clashApis: ClashApiProfile[] = savedProfiles.length > 0
    ? savedProfiles
    : [createClashApiProfile('默认 API', legacyUrl, legacySecret)]

  const activeClashApiId =
    typeof raw?.activeClashApiId === 'string'
    && clashApis.some((api) => api.id === raw.activeClashApiId)
      ? raw.activeClashApiId
      : clashApis[0].id

  return {
    clashApis,
    activeClashApiId,
    singboxPath: normalizePath(raw?.singboxPath),
    coreInstallPath: normalizePath(raw?.coreInstallPath),
    configPath: normalizePath(raw?.configPath),
    rulesDir: normalizePath(raw?.rulesDir),
    workingDir: normalizePath(raw?.workingDir),
    theme: ['auto', 'light', 'dark'].includes(raw?.theme) ? raw.theme : 'light',
    glassMode: raw?.glassMode === 'clear' ? 'clear' : 'blur',
    latencyTestUrl: typeof raw?.latencyTestUrl === 'string' && raw.latencyTestUrl
      ? raw.latencyTestUrl
      : 'https://www.gstatic.com/generate_204',
    ipv6TestEnabled: typeof raw?.ipv6TestEnabled === 'boolean' ? raw.ipv6TestEnabled : false,
    groupTestUrls: raw?.groupTestUrls && typeof raw.groupTestUrls === 'object' && !Array.isArray(raw.groupTestUrls)
      ? Object.fromEntries(
          Object.entries(raw.groupTestUrls as Record<string, unknown>).filter((e): e is [string, string] => typeof e[1] === 'string' && e[1].length > 0)
        )
      : {},
    rootMode: typeof raw?.rootMode === 'boolean' ? raw.rootMode : false,
    setupCompleted: typeof raw?.setupCompleted === 'boolean' ? raw.setupCompleted : false,
    coreUpdateSource: raw?.coreUpdateSource === 'ref1nd' || raw?.coreUpdateSource === 'custom'
      ? raw.coreUpdateSource
      : 'official',
    coreUpdateCustomRepo: typeof raw?.coreUpdateCustomRepo === 'string' ? raw.coreUpdateCustomRepo.trim() : '',
    coreUpdateChannel: raw?.coreUpdateChannel === 'testing' ? 'testing' : 'stable',
    coreUpdateMirror: typeof raw?.coreUpdateMirror === 'string' ? raw.coreUpdateMirror.trim() : '',
    panelAutoCheckUpdate: typeof raw?.panelAutoCheckUpdate === 'boolean' ? raw.panelAutoCheckUpdate : true,
  }
}

let storedConfig: string | null = null

function loadConfig(): AppConfig {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    storedConfig = saved
    if (saved) {
      return normalizeConfig(JSON.parse(saved))
    }
  } catch { }
  return normalizeConfig({})
}

const config = ref<AppConfig>(loadConfig())
let syncingStorage = false

function refreshConfigFromStorage() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved === storedConfig) return
    const nextConfig = normalizeConfig(saved ? JSON.parse(saved) : {})
    storedConfig = saved
    syncingStorage = true
    config.value = nextConfig
  } catch {
  } finally {
    syncingStorage = false
  }
}

window.addEventListener('storage', (event) => {
  if (event.storageArea !== localStorage) return
  if (event.key !== STORAGE_KEY && event.key !== null) return
  refreshConfigFromStorage()
})

// auto 主题：跟随系统明亮/暗黑模式
const systemDarkQuery = window.matchMedia('(prefers-color-scheme: dark)')
const systemDark = ref(systemDarkQuery.matches)
systemDarkQuery.addEventListener('change', (e) => {
  systemDark.value = e.matches
})

const resolvedTheme = computed(() =>
  config.value.theme === 'auto'
    ? (systemDark.value ? 'dark' : 'light')
    : config.value.theme,
)

function applyTheme(theme: string) {
  document.documentElement.setAttribute('data-theme', theme)
}

applyTheme(resolvedTheme.value)

watch(resolvedTheme, (val) => {
  applyTheme(val)
})

function applyGlassMode(mode: AppConfig['glassMode']) {
  document.documentElement.setAttribute('data-glass', mode)
}

applyGlassMode(config.value.glassMode)

watch(() => config.value.glassMode, (val) => {
  applyGlassMode(val)
})

watch(config, (val) => {
  if (syncingStorage) return
  const saved = JSON.stringify(val)
  localStorage.setItem(STORAGE_KEY, saved)
  storedConfig = saved
}, { deep: true, flush: 'sync' })

export const rootReady = setRootEnabled(config.value.rootMode).catch(() => {})
watch(() => config.value.rootMode, (val) => {
  setRootEnabled(val).catch(() => {})
})

export function useConfigStore() {
  const clashApis = computed(() => config.value.clashApis)
  const activeClashApiId = computed(() => config.value.activeClashApiId)
  const activeClashApi = computed(() =>
    config.value.clashApis.find((api) => api.id === config.value.activeClashApiId)
    ?? config.value.clashApis[0],
  )
  const clashApiUrl = computed(() => activeClashApi.value?.url ?? '')
  const clashApiSecret = computed(() => activeClashApi.value?.secret ?? '')
  const clashApiEndpoint = computed(() => `${clashApiUrl.value}\n${clashApiSecret.value}`)

  function updateConfig(partial: Partial<AppConfig>) {
    config.value = normalizeConfig({ ...config.value, ...partial })
  }

  function setActiveClashApi(id: string) {
    if (config.value.clashApis.some((api) => api.id === id)) {
      config.value.activeClashApiId = id
    }
  }

  function addClashApi(name: string, url: string, secret: string): string {
    const profile = createClashApiProfile(name, url, secret)
    config.value.clashApis.push(profile)
    return profile.id
  }

  function updateActiveClashApi(partial: Partial<Omit<ClashApiProfile, 'id'>>) {
    const current = activeClashApi.value
    if (!current) return
    if (typeof partial.name === 'string') current.name = partial.name
    if (typeof partial.url === 'string') current.url = partial.url
    if (typeof partial.secret === 'string') current.secret = partial.secret
  }

  function removeClashApi(id: string): boolean {
    if (config.value.clashApis.length <= 1) return false
    const index = config.value.clashApis.findIndex((api) => api.id === id)
    if (index === -1) return false
    const removingActive = config.value.activeClashApiId === id
    config.value.clashApis.splice(index, 1)
    if (removingActive) {
      config.value.activeClashApiId = config.value.clashApis[0].id
    }
    return true
  }

  function setSingleClashApi(url: string, secret: string, name = '默认 API') {
    const profile = createClashApiProfile(name, url, secret)
    config.value.clashApis = [profile]
    config.value.activeClashApiId = profile.id
  }

  return {
    config,
    resolvedTheme,
    clashApis,
    activeClashApi,
    activeClashApiId,
    clashApiUrl,
    clashApiSecret,
    clashApiEndpoint,
    refreshConfigFromStorage,
    updateConfig,
    setActiveClashApi,
    addClashApi,
    updateActiveClashApi,
    removeClashApi,
    setSingleClashApi,
  }
}
