import { defineConfig } from 'vitepress'

export default defineConfig({
  lang: 'zh-CN',
  title: 'Semantic Installer',
  description: '基于 Rust + egui 的类 HTML 语义化安装引擎 —— 完全开源，让安装界面像写网页一样简单',
  // GitHub Pages 项目站点路径（仓库级站点必须，否则静态资源 404）
  base: '/Semantic-Installer/',
  cleanUrls: true,
  lastUpdated: true,

  head: [
    ['link', { rel: 'icon', href: '/logo.svg' }],
  ],

  themeConfig: {
    logo: '/logo.svg',
    nav: [
      { text: '首页', link: '/' },
      { text: '快速开始', link: '/quick-start' },
      {
        text: '文档',
        items: [
          { text: '总体架构', link: '/architecture' },
          { text: '标记参考', link: '/markup-reference' },
          { text: '二次开发', link: '/secondary-development' },
          { text: '测试与验证', link: '/testing' },
        ],
      },
      { text: 'GitHub', link: 'https://github.com/bainano/Semantic-Installer' },
    ],

    sidebar: [
      {
        text: '开始',
        items: [
          { text: '快速开始', link: '/quick-start' },
          { text: '示例工程', link: '/examples' },
        ],
      },
      {
        text: '开发者文档',
        items: [
          { text: '总体架构', link: '/architecture' },
          { text: '标记参考', link: '/markup-reference' },
          { text: '二次开发', link: '/secondary-development' },
          { text: '测试与验证', link: '/testing' },
        ],
      },
    ],

    search: {
      provider: 'local',
      options: {
        translations: {
          button: { buttonText: '搜索文档', buttonAriaLabel: '搜索文档' },
          modal: {
            displayDetails: '显示详细列表',
            resetButtonTitle: '清除搜索条件',
            backButtonTitle: '返回',
            noResultsText: '未找到相关结果',
            footer: { selectText: '选择', navigateText: '切换', closeText: '关闭' },
          },
        },
      },
    },

    outline: { label: '本页目录', level: [2, 3] },
    docFooter: { prev: '上一页', next: '下一页' },
    returnToTopLabel: '返回顶部',
    sidebarMenuLabel: '目录',
    darkModeSwitchLabel: '外观',
    lightModeSwitchTitle: '切换到浅色模式',
    darkModeSwitchTitle: '切换到深色模式',
    lastUpdated: { text: '最后更新于' },

    footer: {
      message: '基于 MIT 协议开源',
      copyright: 'Copyright © 2026 Semantic Installer Contributors',
    },
  },
})
