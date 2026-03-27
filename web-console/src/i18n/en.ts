export const en: Record<string, string> = {
  // Nav
  'nav.sessions': 'Sessions',
  'nav.test': 'Session Test',
  'nav.audio': 'Audio Debug',
  'nav.transcript': 'Transcript Debug',
  'nav.agent': 'Agent Debug',
  'nav.tts': 'TTS Debug',
  'nav.events': 'Event Timeline',
  'nav.replay': 'Replay',
  'nav.metrics': 'Metrics',
  'nav.admin': 'Admin',

  // Common
  'common.loading': 'Loading...',
  'common.noData': 'No data',
  'common.refresh': 'Refresh',
  'common.close': 'Close',
  'common.create': 'Create',
  'common.delete': 'Delete',
  'common.save': 'Save',
  'common.cancel': 'Cancel',
  'common.confirm': 'Confirm',
  'common.actions': 'Actions',
  'common.status': 'Status',
  'common.time': 'Time',
  'common.language': 'Language',

  // Login
  'login.title': 'PRX Voice Engine',
  'login.subtitle': 'Industrial-grade Real-time Voice AI Platform',
  'login.username': 'Username',
  'login.password': 'Password',
  'login.submit': 'Sign In',
  'login.error': 'Invalid username or password',
  'login.remember': 'Remember me',

  // Sessions
  'sessions.title': 'Sessions',
  'sessions.new': '+ New Session',
  'sessions.id': 'Session ID',
  'sessions.state': 'State',
  'sessions.channel': 'Channel',
  'sessions.language': 'Language',
  'sessions.turn': 'Turn',
  'sessions.noSessions': 'No active sessions',
  'sessions.close': 'Close',

  // Session Test
  'test.title': 'Session Test',
  'test.provider': 'Provider',
  'test.start': 'Start Session',
  'test.interrupt': 'Interrupt',
  'test.eventStream': 'Event Stream',
  'test.noEvents': 'No events yet. Start a session to see events.',

  // Audio Debug
  'audio.title': 'Audio Debug',
  'audio.sampleRate': 'Sample Rate',
  'audio.channels': 'Channels',
  'audio.encoding': 'Encoding',
  'audio.vadActive': 'VAD Active',
  'audio.droppedFrames': 'Dropped Frames',
  'audio.bufferDepth': 'Buffer Depth',
  'audio.inputLevel': 'Input Level (Microphone)',
  'audio.outputLevel': 'Output Level (Playback)',
  'audio.waveform': 'Waveform (Audio Input)',
  'audio.connectHint': 'Connect to a session to see live audio waveform. VAD trigger points shown as vertical markers.',

  // Transcript
  'transcript.title': 'Transcript Debug',
  'transcript.final': 'Final Transcripts',
  'transcript.partial': 'Partial Transcripts (Revisions)',
  'transcript.noFinal': 'No final transcripts yet',
  'transcript.noPartial': 'No partials captured',
  'transcript.vadEvents': 'VAD Events',
  'transcript.noVad': 'No VAD events captured',
  'transcript.speechStart': 'Speech Start',
  'transcript.speechEnd': 'Speech End',

  // Agent
  'agent.title': 'Agent Debug',
  'agent.thinking': 'Thinking Events',
  'agent.tokens': 'Token Chunks',
  'agent.completions': 'Completions',
  'agent.errors': 'Errors',
  'agent.responses': 'Agent Responses',
  'agent.noResponses': 'No responses yet. Start a session.',
  'agent.tokenStream': 'Token Stream (Last 50)',
  'agent.noTokens': 'No tokens streamed yet',

  // TTS
  'tts.title': 'TTS Debug',
  'tts.segmentsQueued': 'Segments Queued',
  'tts.chunksReady': 'Chunks Ready',
  'tts.playbackEvents': 'Playback Events',
  'tts.stops': 'Stops/Flushes',
  'tts.segmentQueue': 'Segment Queue',
  'tts.noSegments': 'No segments queued',
  'tts.playbackTimeline': 'Playback Timeline',
  'tts.noPlayback': 'No playback events',

  // Events
  'events.title': 'Event Timeline',
  'events.noEvents': 'No events captured. Start a session from the Session Test page.',

  // Replay
  'replay.title': 'Session Replay',
  'replay.play': 'Replay',
  'replay.stop': 'Stop',
  'replay.noEvents': 'No events to replay',
  'replay.noEventsHint': 'Run a session from the Session Test page first.',

  // Metrics
  'metrics.title': 'Metrics',
  'metrics.noMetrics': 'No metrics available yet. Create some sessions first.',
  'metrics.raw': 'Raw Prometheus Output',

  // Admin
  'admin.title': 'Administration',
  'admin.auditLog': 'Audit Log',
  'admin.billing': 'Billing',
  'admin.action': 'Action',
  'admin.target': 'Target',
  'admin.result': 'Result',
  'admin.reason': 'Reason',
  'admin.noAudit': 'No audit records',
  'admin.totalEntries': 'Total Billing Entries',

  // Logout
  'logout': 'Sign Out',
}
