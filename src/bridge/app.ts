import { invoke } from '@tauri-apps/api/core'

export interface SystemInsets {
  top: number
  right: number
  bottom: number
  left: number
}

export async function checkRootAccess(): Promise<boolean> {
  return invoke<boolean>('root_check')
}

export async function setRootEnabled(enabled: boolean): Promise<void> {
  return invoke('set_root_enabled', { enabled })
}

export async function setSystemBars(dark: boolean, color?: string): Promise<void> {
  return invoke('set_system_bars', { dark, color })
}

export async function getSystemInsets(): Promise<SystemInsets | null> {
  return invoke<SystemInsets | null>('system_insets')
}
