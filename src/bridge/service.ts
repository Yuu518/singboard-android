import { invoke } from '@tauri-apps/api/core'
import type { ServiceStatus } from '@/types'

export async function queryServiceStatus(): Promise<ServiceStatus> {
  return invoke<ServiceStatus>('service_status')
}

export async function startService(): Promise<void> {
  return invoke('service_start')
}

export async function stopService(): Promise<void> {
  return invoke('service_stop')
}

export async function restartService(): Promise<void> {
  return invoke('service_restart')
}

export async function installService(
  singboxPath: string,
  configPath: string,
  workingDir: string,
): Promise<void> {
  return invoke('service_install', { singboxPath, configPath, workingDir })
}

// 返回 'not_installed' | 'ok' | 'updated'
export async function syncServiceComponent(): Promise<string> {
  return invoke<string>('service_component_sync')
}

export async function readServiceErrorLog(): Promise<string> {
  return invoke<string>('service_error_log')
}

export async function bootHookExists(): Promise<boolean> {
  return invoke<boolean>('service_boot_hook_exists')
}

export async function createBootHook(): Promise<void> {
  return invoke('service_create_boot_hook')
}

export async function deleteBootHook(): Promise<void> {
  return invoke('service_delete_boot_hook')
}
