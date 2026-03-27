export const zh: Record<string, string> = {
  // Nav
  'nav.sessions': '会话管理',
  'nav.test': '会话测试',
  'nav.audio': '音频调试',
  'nav.transcript': '转写调试',
  'nav.agent': 'Agent 调试',
  'nav.tts': 'TTS 调试',
  'nav.events': '事件时间线',
  'nav.replay': '会话回放',
  'nav.metrics': '监控指标',
  'nav.admin': '系统管理',

  // Common
  'common.loading': '加载中...',
  'common.noData': '暂无数据',
  'common.refresh': '刷新',
  'common.close': '关闭',
  'common.create': '创建',
  'common.delete': '删除',
  'common.save': '保存',
  'common.cancel': '取消',
  'common.confirm': '确认',
  'common.actions': '操作',
  'common.status': '状态',
  'common.time': '时间',
  'common.language': '语言',

  // Login
  'login.title': 'PRX Voice Engine',
  'login.subtitle': '工业级实时语音 AI 平台',
  'login.username': '用户名',
  'login.password': '密码',
  'login.submit': '登录',
  'login.error': '用户名或密码错误',
  'login.remember': '记住登录',

  // Sessions
  'sessions.title': '会话管理',
  'sessions.new': '+ 新建会话',
  'sessions.id': '会话 ID',
  'sessions.state': '状态',
  'sessions.channel': '通道',
  'sessions.language': '语言',
  'sessions.turn': '轮次',
  'sessions.noSessions': '暂无活跃会话',
  'sessions.close': '关闭',

  // Session Test
  'test.title': '会话测试',
  'test.provider': '供应商',
  'test.start': '开始会话',
  'test.interrupt': '打断',
  'test.eventStream': '事件流',
  'test.noEvents': '暂无事件。开始会话后可查看事件流。',

  // Audio Debug
  'audio.title': '音频调试',
  'audio.sampleRate': '采样率',
  'audio.channels': '声道数',
  'audio.encoding': '编码格式',
  'audio.vadActive': 'VAD 激活',
  'audio.droppedFrames': '丢帧数',
  'audio.bufferDepth': '缓冲深度',
  'audio.inputLevel': '输入电平（麦克风）',
  'audio.outputLevel': '输出电平（播放）',
  'audio.waveform': '波形（音频输入）',
  'audio.connectHint': '连接会话以查看实时音频波形。VAD 触发点显示为垂直标记。',

  // Transcript
  'transcript.title': '转写调试',
  'transcript.final': '最终转写',
  'transcript.partial': '部分转写（修订流）',
  'transcript.noFinal': '暂无最终转写',
  'transcript.noPartial': '暂无部分转写',
  'transcript.vadEvents': 'VAD 事件',
  'transcript.noVad': '暂无 VAD 事件',
  'transcript.speechStart': '语音开始',
  'transcript.speechEnd': '语音结束',

  // Agent
  'agent.title': 'Agent 调试',
  'agent.thinking': '思考事件',
  'agent.tokens': 'Token 块',
  'agent.completions': '完成数',
  'agent.errors': '错误数',
  'agent.responses': 'Agent 响应',
  'agent.noResponses': '暂无响应。请先开始会话。',
  'agent.tokenStream': 'Token 流（最近 50 条）',
  'agent.noTokens': '暂无 Token 流',

  // TTS
  'tts.title': 'TTS 调试',
  'tts.segmentsQueued': '排队分段',
  'tts.chunksReady': '就绪块',
  'tts.playbackEvents': '播放事件',
  'tts.stops': '停止/刷新',
  'tts.segmentQueue': '分段队列',
  'tts.noSegments': '暂无排队分段',
  'tts.playbackTimeline': '播放时间线',
  'tts.noPlayback': '暂无播放事件',

  // Events
  'events.title': '事件时间线',
  'events.noEvents': '暂无事件。请先从会话测试页面开始会话。',

  // Replay
  'replay.title': '会话回放',
  'replay.play': '回放',
  'replay.stop': '停止',
  'replay.noEvents': '暂无可回放的事件',
  'replay.noEventsHint': '请先从会话测试页面运行一个会话。',

  // Metrics
  'metrics.title': '监控指标',
  'metrics.noMetrics': '暂无可用指标。请先创建一些会话。',
  'metrics.raw': '原始 Prometheus 输出',

  // Admin
  'admin.title': '系统管理',
  'admin.auditLog': '审计日志',
  'admin.billing': '计费',
  'admin.action': '操作',
  'admin.target': '目标',
  'admin.result': '结果',
  'admin.reason': '原因',
  'admin.noAudit': '暂无审计记录',
  'admin.totalEntries': '计费条目总数',

  // Logout
  'logout': '退出登录',
}
