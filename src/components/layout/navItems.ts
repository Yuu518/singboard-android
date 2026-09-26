import type { AsyncComponentLoader } from 'vue'

export interface NavItem {
  path: string
  label: string
  icon: string[]
  fill?: boolean
  gate?: string
  load: AsyncComponentLoader
}

export const navItems: NavItem[] = [
  {
    path: '/home',
    load: () => import('@/views/HomePage.vue'),
    label: '首页',
    icon: [
      'M3 10.5 12 3l9 7.5',
      'M5 9v10.5a1.5 1.5 0 0 0 1.5 1.5H10v-6h4v6h3.5a1.5 1.5 0 0 0 1.5-1.5V9',
    ],
  },
  {
    path: '/proxies',
    fill: true,
    load: () => import('@/views/ProxiesPage.vue'),
    gate: '代理信息',
    label: '代理',
    icon: [
      'M6 3v12',
      'M18 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6z',
      'M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6z',
      'M18 9a9 9 0 0 1-9 9',
    ],
  },
  {
    path: '/connections',
    fill: true,
    load: () => import('@/views/ConnectionsPage.vue'),
    gate: '连接信息',
    label: '连接',
    icon: [
      'M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71',
      'M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71',
    ],
  },
  {
    path: '/logs',
    fill: true,
    load: () => import('@/views/LogsPage.vue'),
    gate: '日志',
    label: '日志',
    icon: [
      'M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z',
      'M14 2v6h6',
      'M16 13H8',
      'M16 17H8',
      'M10 9H8',
    ],
  },
  {
    path: '/rules',
    fill: true,
    load: () => import('@/views/RulesPage.vue'),
    gate: '规则信息',
    label: '规则',
    icon: ['M8 6h13', 'M8 12h13', 'M8 18h13', 'M3.5 6h.01', 'M3.5 12h.01', 'M3.5 18h.01'],
  },
  {
    path: '/settings',
    load: () => import('@/views/SettingsPage.vue'),
    label: '设置',
    icon: [
      'M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z',
      'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z',
    ],
  },
]

