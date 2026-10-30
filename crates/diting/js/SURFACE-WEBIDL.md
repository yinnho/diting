# JS 表面 × WebIDL 差集（生成件）

> 生成：`scripts/surface_webidl_diff.py`（见脚本头部三步）。基准 = webref `curated` 的 Window 暴露接口 + MDN BCD 5.6.30（Chrome 已发布判据）。
> **缺口 = 标准成员我们没有 且 Chrome 已发布**；实验/带旗标/已移除的不计。成员按**实例**探测（事件/请求类垫片把成员挂实例，不挂原型）。

## 接口级

| 项 | 数 |
|---|---|
| 标准 Window 暴露接口 | 428 |
| 有 | 357 |
| 缺（Chrome 已发布） | 0 |
| 缺（实验/未发布/他引擎） | 71 |
| 多（非标准暴露，去 JS 内建） | 39 |

**缺·Chrome 已发布**：无

**缺·其余（不排期）**：AmbientLightSensor, AutofillEvent, BluetoothAdvertisingEvent, BluetoothDataFilter, BluetoothLEScanFilter, BluetoothManufacturerDataFilter, BluetoothServiceDataFilter, BufferedChangeEvent, CSSColor, CSSHSL, CSSHWB, CSSLCH, CSSLab, CSSOKLCH, CSSOKLab, CSSParserAtRule, CSSParserBlock, CSSParserDeclaration, CSSParserFunction, CSSParserQualifiedRule, CSSRGB, CaptureActionEvent, CapturedMouseEvent, DataCue, DeviceChangeEvent, Exception, FaceDetector, Global, GroupEffect, HTMLInstallElement, HTMLPortalElement, HandwritingStroke, InstallResultEvent, Instance, LanguageModelToolCall, LanguageModelToolError, LanguageModelToolSuccess, MLGraphBuilder, Magnetometer, ManagedMediaSource, MediaStreamTrackAudioSourceNode, MediaStreamTrackHandle, Memory, Module, NDEFMessage, NDEFReader, NDEFReadingEvent, NDEFRecord, NavigationEvent, PointerTimeline, PortalActivateEvent, ProximitySensor, RTCIdentityAssertion, RTCRtpSFrameDecryptor, RTCRtpSFrameEncryptor, SFrameDecryptorStream, SFrameEncryptorStream, SFrameTransformErrorEvent, SequenceEffect, ShadowAnimation, Table, Tag, TextDetector, ToolActivatedEvent, ToolCancelEvent, UncalibratedMagnetometer, VTTRegion, ValueEvent, WorkletAnimation, XRGPUBinding, XRMediaBinding

**多（逐个决定留/藏）**：Audio, CSPViolationReportBody, CSS, CSSPositionValue, DOMError, FeaturePolicy, FileSystemObserver, GPUBufferUsage, GPUColorWrite, GPUMapMode, GPUShaderStage, GPUTextureUsage, HTMLDocument, Image, IntegrityViolationReportBody, InterestEvent, MediaStreamEvent, MediaStreamTrackGenerator, NodeFilter, NodeRange, OpaqueRange, Option, PaymentAddress, PerformanceLongTaskTiming, ProtectedAudience, ReportBody, SVGMatrix, SVGPoint, SVGRect, TaskAttributionTiming, TimelineTrigger, TimelineTriggerRange, TimelineTriggerRangeList, VirtualKeyboardGeometryChangeEvent, WebKitCSSMatrix, WebKitMutationObserver, WebSocketError, WebSocketStream, XRDOMOverlayState

> 注：Audio/Image/Option 是合法命名构造器；CSS/NodeFilter 在 IDL 里是 namespace/callback，属标准——判多时先排除。

## 成员级缺口（Chrome 已发布 · 已探到实例的接口）

已核 199 个接口；有缺口 55 个；缺口成员合计 224。

| 接口 | 探法 | 缺/标准成员 | 缺失成员 |
|---|---|---|---|
| Document | document | 31/294 | alinkColor, anchors, applets, bgColor, captureEvents, caretPositionFromPoint, clear, createExpression, createNSResolver, designMode, embeds, exitFullscreen, exitPointerLock, fgColor, fragmentDirective, fullscreen, fullscreenElement, fullscreenEnabled, hasPrivateToken, hasRedemptionRecord, hasUnpartitionedCookieAccess, lastModified, linkColor, plugins, pointerLockElement, prerendering, releaseEvents, rootElement, timeline, vlinkColor, wasDiscarded |
| Animation | new | 19/28 | cancel, commitStyles, currentTime, effect, finish, finished, id, pause, pending, persist, play, playState, playbackRate, ready, replaceState, reverse, startTime, timeline, updatePlaybackRate |
| RTCIceCandidate | new | 16/17 | address, component, foundation, port, priority, protocol, relatedAddress, relatedPort, relayProtocol, sdpMLineIndex, sdpMid, tcpType, toJSON, type, url, usernameFragment |
| RTCPeerConnection | new | 15/47 | addTrack, addTransceiver, canTrickleIceCandidates, currentLocalDescription, currentRemoteDescription, getConfiguration, getReceivers, getSenders, getTransceivers, pendingLocalDescription, pendingRemoteDescription, removeTrack, restartIce, sctp, setConfiguration |
| EditContext | new | 11/20 | attachedElements, characterBounds, characterBoundsRangeStart, selectionEnd, selectionStart, text, updateCharacterBounds, updateControlBounds, updateSelection, updateSelectionBounds, updateText |
| VideoFrame | new | 10/17 | allocationSize, clone, codedHeight, codedRect, codedWidth, colorSpace, copyTo, displayHeight, displayWidth, visibleRect |
| IntersectionObserverEntry | new | 8/8 | boundingClientRect, intersectionRatio, intersectionRect, isIntersecting, isVisible, rootBounds, target, time |
| XSLTProcessor | new | 8/8 | clearParameters, getParameter, importStylesheet, removeParameter, reset, setParameter, transformToDocument, transformToFragment |
| Request | new | 7/27 | bodyUsed, destination, duplex, integrity, isHistoryNavigation, keepalive, targetAddressSpace |
| ViewTimeline | new | 7/8 | axis, currentTime, duration, endOffset, source, startOffset, subject |
| VideoColorSpace | new | 5/5 | fullRange, matrix, primaries, toJSON, transfer |
| WebTransportError | new | 5/30 | code, message, name, source, streamErrorCode |
| ScrollTimeline | new | 4/5 | axis, currentTime, duration, source |
| TextFormat | new | 4/4 | rangeEnd, rangeStart, underlineStyle, underlineThickness |
| XRRigidTransform | new | 4/4 | inverse, matrix, orientation, position |
| AudioContext | new | 3/46 | audioWorklet, setSinkId, sinkId |
| CloseWatcher | new | 3/9 | close, destroy, requestClose |
| DataTransfer | new | 3/9 | clearData, getData, setData |
| FontFace | new | 3/19 | ascentOverride, descentOverride, lineGapOverride |
| IdentityCredentialError | new | 3/30 | code, message, name |
| IdleDetector | new | 3/8 | screenState, start, userState |
| ProcessingInstruction | new | 3/79 | nextElementSibling, previousElementSibling, sheet |
| QuotaExceededError | new | 3/30 | code, message, name |
| TaskController | new | 3/3 | abort, setPriority, signal |
| Text | createTextNode | 3/77 | assignedSlot, nextElementSibling, previousElementSibling |
| XPathEvaluator | new | 3/3 | createExpression, createNSResolver, evaluate |
| XRRay | new | 3/3 | direction, matrix, origin |
| Comment | new | 2/70 | nextElementSibling, previousElementSibling |
| DocumentTimeline | new | 2/3 | currentTime, duration |
| HTMLCanvasElement | createElement | 2/371 | captureStream, transferControlToOffscreen |
| HashChangeEvent | new | 2/24 | newURL, oldURL |
| Highlight | new | 2/2 | priority, type |
| WebTransport | new | 2/19 | incomingBidirectionalStreams, incomingUnidirectionalStreams |
| BarcodeDetector | new | 1/1 | detect |
| CaptureController | new | 1/13 | setFocusBehavior |
| ClipboardEvent | new | 1/23 | clipboardData |
| DocumentFragment | new | 1/62 | childElementCount |
| File | new | 1/11 | webkitRelativePath |
| HTMLIFrameElement | createElement | 1/388 | privateToken |
| Headers | new | 1/6 | getSetCookie |
| ImageData | new | 1/5 | colorSpace |
| InputDeviceCapabilities | new | 1/2 | firesTouchEvents |
| MediaMetadata | new | 1/5 | chapterInfo |
| MediaSource | new | 1/18 | handle |
| MediaStream | new | 1/15 | getTrackById |
| MessageEvent | new | 1/28 | initMessageEvent |
| Notification | new | 1/26 | image |
| OfflineAudioContext | new | 1/37 | audioWorklet |
| Path2D | new | 1/11 | roundRect |
| PopStateEvent | new | 1/24 | hasUAVisualTransition |
| RTCError | new | 1/34 | httpRequestStatusCode |
| RTCSessionDescription | new | 1/3 | toJSON |
| SpeechSynthesisUtterance | new | 1/17 | voice |
| SubmitEvent | new | 1/23 | submitter |
| XMLHttpRequest | new | 1/36 | setPrivateToken |

## 原型级探测（实例造不出，成员未核实）

AnalyserNode, AnimationPlaybackEvent, AnimationTrigger, AudioBufferSourceNode, AudioData, AudioDecoder, AudioEncoder, AudioProcessingEvent, AudioWorkletNode, BeforeInstallPromptEvent, BiquadFilterNode, BlobEvent, ByteLengthQueuingStrategy, CSSKeywordValue, CSSMathClamp, CSSMathInvert, CSSMathMax, CSSMathMin, CSSMathNegate, CSSMathProduct, CSSMathSum, CSSMatrixComponent, CSSPerspective, CSSRotate, CSSScale, CSSSkew, CSSSkewX, CSSSkewY, CSSTransformValue, CSSTranslate, CSSUnitValue, CSSUnparsedValue, CSSVariableReferenceValue, ChannelMergerNode, ChannelSplitterNode, CharacterBoundsUpdateEvent, ClipboardItem, CommandEvent, CompressionStream, ConstantSourceNode, ConvolverNode, CookieChangeEvent, CountQueuingStrategy, DecompressionStream, DelayNode, DeviceMotionEvent, DeviceOrientationEvent, DocumentPictureInPictureEvent, DragEvent, DynamicsCompressorNode, EncodedAudioChunk, EncodedVideoChunk, FederatedCredential, FencedFrameConfig, FontFaceSetLoadEvent, FormDataEvent, GPUInternalError, GPUOutOfMemoryError, GPUPipelineError, GPUUncapturedErrorEvent, GPUValidationError, GainNode, GamepadEvent, HIDConnectionEvent, HIDInputReportEvent, HTMLCameraElement, HTMLFencedFrameElement, HTMLGeolocationElement, HTMLHtmlElement, HTMLMicrophoneElement, HTMLSelectedContentElement, HTMLUserMediaElement, IDBVersionChangeEvent, IIRFilterNode, ImageCapture, ImageDecoder, KeyframeEffect, MIDIConnectionEvent, MIDIMessageEvent, MediaElementAudioSourceNode, MediaEncryptedEvent, MediaKeyMessageEvent, MediaQueryListEvent, MediaRecorder, MediaStreamAudioDestinationNode, MediaStreamAudioSourceNode, MediaStreamTrackEvent, NavigateEvent, NavigationCurrentEntryChangeEvent, Observable, OfflineAudioCompletionEvent, OscillatorNode, OverconstrainedError, PageRevealEvent, PageSwapEvent, PageTransitionEvent, PannerNode, PasswordCredential, PaymentMethodChangeEvent, PaymentRequest, PaymentRequestUpdateEvent, PerformanceObserver, PictureInPictureEvent, PresentationConnectionAvailableEvent, PresentationConnectionCloseEvent, PresentationRequest, PressureObserver, Profiler, PromiseRejectionEvent, RTCDTMFToneChangeEvent, RTCDataChannelEvent, RTCEncodedAudioFrame, RTCEncodedVideoFrame, RTCErrorEvent, RTCIceTransport, ReadableStreamBYOBReader, ReadableStreamDefaultReader, ReportingObserver, SecurityPolicyViolationEvent, SensorErrorEvent, SnapEvent, SpeechSynthesisErrorEvent, SpeechSynthesisEvent, StaticRange, StereoPannerNode, TaskPriorityChangeEvent, TextFormatUpdateEvent, TextUpdateEvent, TrackEvent, USBAlternateInterface, USBConfiguration, USBConnectionEvent, USBEndpoint, USBInTransferResult, USBInterface, USBIsochronousInTransferPacket, USBIsochronousInTransferResult, USBIsochronousOutTransferPacket, USBIsochronousOutTransferResult, USBOutTransferResult, VTTCue, VideoDecoder, VideoEncoder, WaveShaperNode, WebGLContextEvent, WebSocket, WindowControlsOverlayGeometryChangeEvent, WritableStreamDefaultWriter, XRInputSourceEvent, XRInputSourcesChangeEvent, XRLayerEvent, XRReferenceSpaceEvent, XRSessionEvent, XRVisibilityMaskChangeEvent, XRWebGLBinding, XRWebGLLayer
