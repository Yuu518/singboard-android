import { afterEach, describe, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-vue'
import { defineComponent, h, type Component } from 'vue'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import { usePageActive } from '@/composables/usePageActive'
import { pagerIndex } from '@/stores/pager'

function page(name: string): Component {
  return defineComponent({
    setup() {
      const active = usePageActive()
      return () => h('div', { 'data-page': name, 'data-active': String(active.value), style: 'height: 2000px' }, name)
    },
  })
}

vi.mock('./navItems', () => ({
  navItems: ['a', 'b', 'c'].map((name) => ({
    path: `/${name}`,
    label: name,
    icon: [],
    load: () => Promise.resolve(page(name)),
  })),
}))

const { default: MainPager } = await import('./MainPager.vue')

const WIDTH = 400

async function mountPager(start: string): Promise<{ router: Router; viewport: HTMLElement }> {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: ['/a', '/b', '/c'].map((path) => ({ path, component: { render: () => null } })),
  })
  await router.push(start)
  const Host = defineComponent({
    setup: () => () => h('div', { style: `width: ${WIDTH}px; height: 600px` }, [h(MainPager)]),
  })
  const screen = render(Host, { global: { plugins: [router] } })
  const viewport = screen.container.querySelector('main') as HTMLElement
  await expect.poll(() => viewport.querySelectorAll('[data-page]').length).toBe(3)
  return { router, viewport }
}

function activePages(viewport: HTMLElement): string[] {
  return [...viewport.querySelectorAll<HTMLElement>('[data-active="true"]')].map((el) => el.dataset.page!)
}

afterEach(() => {
  document.body.innerHTML = ''
})

describe('main pager', () => {
  it('opens on the routed page and keeps every page mounted', async () => {
    const { viewport } = await mountPager('/b')

    await expect.poll(() => viewport.scrollLeft).toBe(WIDTH)
    expect(pagerIndex.value).toBe(1)
    expect(activePages(viewport)).toEqual(['b'])
    const sections = viewport.querySelectorAll('section')
    expect(sections[0].hasAttribute('inert')).toBe(true)
    expect(sections[1].hasAttribute('inert')).toBe(false)
  })

  it('slides to a page when the route changes without remounting it', async () => {
    const { router, viewport } = await mountPager('/a')
    const pageC = viewport.querySelector('[data-page="c"]')

    await router.push('/c')

    await expect.poll(() => viewport.scrollLeft, { timeout: 3000 }).toBe(WIDTH * 2)
    await expect.poll(() => activePages(viewport)).toEqual(['c'])
    expect(viewport.querySelector('[data-page="c"]')).toBe(pageC)
  })

  it('updates the route after a swipe settles on another page', async () => {
    const { router, viewport } = await mountPager('/a')
    await expect.poll(() => pagerIndex.value).toBe(0)

    viewport.scrollTo({ left: WIDTH, behavior: 'instant' })

    await expect.poll(() => router.currentRoute.value.path).toBe('/b')
    expect(pagerIndex.value).toBe(1)
    await expect.poll(() => activePages(viewport)).toEqual(['b'])
  })
})
