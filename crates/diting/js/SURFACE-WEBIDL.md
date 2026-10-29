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

已核 196 个接口；有缺口 140 个；缺口成员合计 992。

| 接口 | 探法 | 缺/标准成员 | 缺失成员 |
|---|---|---|---|
| HTMLVideoElement | createElement | 36/431 | addTextTrack, attributeStyleMap, autoplay, cancelVideoFrameCallback, captureStream, computedStyleMap, controls, crossOrigin, currentSrc, defaultMuted, defaultPlaybackRate, disableRemotePlayback, error, getVideoPlaybackQuality, height, loop, mediaKeys, muted, part, playsInline, poster, preload, preservesPitch, remote, requestFullscreen, requestPointerLock, requestVideoFrameCallback, setMediaKeys, setSinkId, sinkId, srcObject, textTracks, videoHeight, videoWidth, volume, width |
| Document | document | 31/294 | alinkColor, anchors, applets, bgColor, captureEvents, caretPositionFromPoint, clear, createExpression, createNSResolver, designMode, embeds, exitFullscreen, exitPointerLock, fgColor, fragmentDirective, fullscreen, fullscreenElement, fullscreenEnabled, hasPrivateToken, hasRedemptionRecord, hasUnpartitionedCookieAccess, lastModified, linkColor, plugins, pointerLockElement, prerendering, releaseEvents, rootElement, timeline, vlinkColor, wasDiscarded |
| HTMLImageElement | createElement | 29/392 | align, alt, attributeStyleMap, border, complete, computedStyleMap, crossOrigin, currentSrc, decode, decoding, fetchPriority, height, hspace, isMap, loading, longDesc, lowsrc, naturalHeight, naturalWidth, part, referrerPolicy, requestFullscreen, requestPointerLock, srcset, useMap, vspace, width, x, y |
| HTMLInputElement | createElement | 28/427 | accept, align, alt, attributeStyleMap, autocomplete, computedStyleMap, defaultChecked, defaultValue, dirName, formAction, formEnctype, formMethod, formTarget, height, list, part, pattern, popoverTargetAction, popoverTargetElement, readOnly, requestFullscreen, requestPointerLock, required, showPicker, useMap, webkitEntries, webkitdirectory, width |
| HTMLAudioElement | createElement | 27/418 | addTextTrack, attributeStyleMap, autoplay, captureStream, computedStyleMap, controls, crossOrigin, currentSrc, defaultMuted, defaultPlaybackRate, disableRemotePlayback, error, loop, mediaKeys, muted, part, preload, preservesPitch, remote, requestFullscreen, requestPointerLock, setMediaKeys, setSinkId, sinkId, srcObject, textTracks, volume |
| HTMLObjectElement | createElement | 26/390 | align, archive, attributeStyleMap, border, checkValidity, code, codeBase, codeType, computedStyleMap, data, declare, getSVGDocument, height, hspace, part, reportValidity, requestFullscreen, requestPointerLock, setCustomValidity, standby, useMap, validationMessage, validity, vspace, width, willValidate |
| HTMLIFrameElement | createElement | 21/388 | align, allow, allowFullscreen, attributeStyleMap, computedStyleMap, credentialless, csp, frameBorder, getSVGDocument, height, loading, longDesc, marginHeight, marginWidth, part, privateToken, referrerPolicy, requestFullscreen, requestPointerLock, scrolling, width |
| Animation | new | 19/28 | cancel, commitStyles, currentTime, effect, finish, finished, id, pause, pending, persist, play, playState, playbackRate, ready, replaceState, reverse, startTime, timeline, updatePlaybackRate |
| ClipboardChangeEvent | new | 18/24 | bubbles, cancelBubble, cancelable, composed, composedPath, currentTarget, defaultPrevented, eventPhase, initEvent, isTrusted, preventDefault, returnValue, srcElement, stopImmediatePropagation, stopPropagation, target, timeStamp, type |
| HTMLMarqueeElement | createElement | 18/377 | attributeStyleMap, behavior, bgColor, computedStyleMap, direction, height, hspace, loop, part, requestFullscreen, requestPointerLock, scrollAmount, scrollDelay, start, stop, trueSpeed, vspace, width |
| HTMLLinkElement | createElement | 17/384 | as, attributeStyleMap, blocking, charset, computedStyleMap, crossOrigin, fetchPriority, hreflang, imageSizes, imageSrcset, integrity, media, part, referrerPolicy, requestFullscreen, requestPointerLock, target |
| PointerEvent | new | 16/65 | altitudeAngle, azimuthAngle, getCoalescedEvents, getModifierState, getPredictedEvents, layerX, layerY, movementX, movementY, persistentDeviceId, sourceCapabilities, tangentialPressure, tiltX, tiltY, twist, which |
| RTCIceCandidate | new | 16/17 | address, component, foundation, port, priority, protocol, relatedAddress, relatedPort, relayProtocol, sdpMLineIndex, sdpMid, tcpType, toJSON, type, url, usernameFragment |
| HTMLScriptElement | createElement | 15/378 | async, attributeStyleMap, blocking, charset, computedStyleMap, crossOrigin, defer, event, fetchPriority, integrity, noModule, part, referrerPolicy, requestFullscreen, requestPointerLock |
| RTCPeerConnection | new | 15/47 | addTrack, addTransceiver, canTrickleIceCandidates, currentLocalDescription, currentRemoteDescription, getConfiguration, getReceivers, getSenders, getTransceivers, pendingLocalDescription, pendingRemoteDescription, removeTrack, restartIce, sctp, setConfiguration |
| Notification | new | 14/26 | actions, badge, body, close, data, dir, icon, lang, renotify, requireInteraction, silent, tag, timestamp, title |
| HTMLAnchorElement | createElement | 13/390 | attributeStyleMap, charset, computedStyleMap, coords, download, hreflang, part, ping, referrerPolicy, requestFullscreen, requestPointerLock, shape, target |
| HTMLAreaElement | createElement | 13/387 | alt, attributeStyleMap, computedStyleMap, coords, download, noHref, part, ping, referrerPolicy, requestFullscreen, requestPointerLock, shape, target |
| HTMLFieldSetElement | createElement | 12/375 | attributeStyleMap, checkValidity, computedStyleMap, elements, part, reportValidity, requestFullscreen, requestPointerLock, setCustomValidity, validationMessage, validity, willValidate |
| HTMLOutputElement | createElement | 12/377 | attributeStyleMap, checkValidity, computedStyleMap, defaultValue, part, reportValidity, requestFullscreen, requestPointerLock, setCustomValidity, validationMessage, validity, willValidate |
| EditContext | new | 11/20 | attachedElements, characterBounds, characterBoundsRangeStart, selectionEnd, selectionStart, text, updateCharacterBounds, updateControlBounds, updateSelection, updateSelectionBounds, updateText |
| HTMLButtonElement | createElement | 11/385 | attributeStyleMap, computedStyleMap, formAction, formEnctype, formMethod, formTarget, part, popoverTargetAction, popoverTargetElement, requestFullscreen, requestPointerLock |
| HTMLFrameElement | createElement | 11/374 | attributeStyleMap, computedStyleMap, frameBorder, longDesc, marginHeight, marginWidth, noResize, part, requestFullscreen, requestPointerLock, scrolling |
| HTMLSelectElement | createElement | 11/388 | attributeStyleMap, autocomplete, computedStyleMap, item, length, namedItem, part, requestFullscreen, requestPointerLock, required, showPicker |
| HTMLTextAreaElement | createElement | 11/394 | attributeStyleMap, autocomplete, computedStyleMap, defaultValue, dirName, part, readOnly, requestFullscreen, requestPointerLock, required, textLength |
| HTMLTrackElement | createElement | 11/375 | attributeStyleMap, computedStyleMap, default, kind, label, part, readyState, requestFullscreen, requestPointerLock, srclang, track |
| HTMLFormElement | createElement | 10/382 | acceptCharset, attributeStyleMap, autocomplete, computedStyleMap, encoding, enctype, part, requestFullscreen, requestPointerLock, target |
| VideoFrame | new | 10/17 | allocationSize, clone, codedHeight, codedRect, codedWidth, colorSpace, copyTo, displayHeight, displayWidth, visibleRect |
| HTMLEmbedElement | createElement | 9/371 | align, attributeStyleMap, computedStyleMap, getSVGDocument, height, part, requestFullscreen, requestPointerLock, width |
| HTMLHRElement | createElement | 9/369 | align, attributeStyleMap, color, computedStyleMap, noShade, part, requestFullscreen, requestPointerLock, width |
| HTMLSourceElement | createElement | 9/371 | attributeStyleMap, computedStyleMap, height, media, part, requestFullscreen, requestPointerLock, srcset, width |
| HTMLTemplateElement | createElement | 9/372 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock, shadowRootClonable, shadowRootDelegatesFocus, shadowRootMode, shadowRootSerializable |
| URLPattern | new | 9/11 | hasRegExpGroups, hash, hostname, password, pathname, port, protocol, search, username |
| Accelerometer | new | 8/15 | activated, hasReading, start, stop, timestamp, x, y, z |
| GravitySensor | new | 8/15 | activated, hasReading, start, stop, timestamp, x, y, z |
| Gyroscope | new | 8/15 | activated, hasReading, start, stop, timestamp, x, y, z |
| HTMLMeterElement | createElement | 8/371 | attributeStyleMap, computedStyleMap, high, low, optimum, part, requestFullscreen, requestPointerLock |
| HTMLOptionElement | createElement | 8/372 | attributeStyleMap, computedStyleMap, defaultSelected, index, label, part, requestFullscreen, requestPointerLock |
| IntersectionObserverEntry | new | 8/8 | boundingClientRect, intersectionRatio, intersectionRect, isIntersecting, isVisible, rootBounds, target, time |
| LinearAccelerationSensor | new | 8/15 | activated, hasReading, start, stop, timestamp, x, y, z |
| SpeechRecognition | new | 8/26 | abort, continuous, grammars, interimResults, lang, maxAlternatives, start, stop |
| XSLTProcessor | new | 8/8 | clearParameters, getParameter, importStylesheet, removeParameter, reset, setParameter, transformToDocument, transformToFragment |
| AbsoluteOrientationSensor | new | 7/14 | activated, hasReading, populateMatrix, quaternion, start, stop, timestamp |
| HTMLCanvasElement | createElement | 7/371 | attributeStyleMap, captureStream, computedStyleMap, part, requestFullscreen, requestPointerLock, transferControlToOffscreen |
| HTMLFontElement | createElement | 7/367 | attributeStyleMap, color, computedStyleMap, face, part, requestFullscreen, requestPointerLock |
| HTMLMetaElement | createElement | 7/369 | attributeStyleMap, computedStyleMap, httpEquiv, media, part, requestFullscreen, requestPointerLock |
| HTMLModElement | createElement | 7/366 | attributeStyleMap, cite, computedStyleMap, dateTime, part, requestFullscreen, requestPointerLock |
| HTMLStyleElement | createElement | 7/369 | attributeStyleMap, blocking, computedStyleMap, media, part, requestFullscreen, requestPointerLock |
| MouseEvent | new | 7/50 | getModifierState, layerX, layerY, movementX, movementY, sourceCapabilities, which |
| RelativeOrientationSensor | new | 7/14 | activated, hasReading, populateMatrix, quaternion, start, stop, timestamp |
| Request | new | 7/27 | bodyUsed, destination, duplex, integrity, isHistoryNavigation, keepalive, targetAddressSpace |
| ViewTimeline | new | 7/8 | axis, currentTime, duration, endOffset, source, startOffset, subject |
| WheelEvent | new | 7/58 | getModifierState, layerX, layerY, movementX, movementY, sourceCapabilities, which |
| HTMLBRElement | createElement | 6/365 | attributeStyleMap, clear, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLBaseElement | createElement | 6/366 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock, target |
| HTMLDListElement | createElement | 6/365 | attributeStyleMap, compact, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLDirectoryElement | createElement | 6/365 | attributeStyleMap, compact, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLLegendElement | createElement | 6/366 | align, attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLMapElement | createElement | 6/366 | areas, attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLMenuElement | createElement | 6/365 | attributeStyleMap, compact, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLOptGroupElement | createElement | 6/366 | attributeStyleMap, computedStyleMap, label, part, requestFullscreen, requestPointerLock |
| HTMLParamElement | createElement | 6/368 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock, valueType |
| HTMLPreElement | createElement | 6/365 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock, width |
| HTMLProgressElement | createElement | 6/368 | attributeStyleMap, computedStyleMap, part, position, requestFullscreen, requestPointerLock |
| HTMLQuoteElement | createElement | 6/365 | attributeStyleMap, cite, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLSlotElement | createElement | 6/368 | assign, attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLTableCaptionElement | createElement | 6/365 | align, attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLTimeElement | createElement | 6/365 | attributeStyleMap, computedStyleMap, dateTime, part, requestFullscreen, requestPointerLock |
| HTMLTitleElement | createElement | 6/365 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock, text |
| KeyboardEvent | new | 6/44 | charCode, getModifierState, isComposing, keyCode, sourceCapabilities, which |
| HTMLBodyElement | createElement | 5/392 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLDataElement | createElement | 5/365 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLDataListElement | createElement | 5/365 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLDetailsElement | createElement | 5/366 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLDialogElement | createElement | 5/371 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLDivElement | createElement | 5/365 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLElement | createElement | 5/364 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLFrameSetElement | createElement | 5/387 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLHeadElement | createElement | 5/364 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLHeadingElement | createElement | 5/365 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLLIElement | createElement | 5/366 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLLabelElement | createElement | 5/367 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLOListElement | createElement | 5/368 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLParagraphElement | createElement | 5/365 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLPictureElement | createElement | 5/364 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLSpanElement | createElement | 5/364 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLTableCellElement | createElement | 5/379 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLTableColElement | createElement | 5/370 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLTableElement | createElement | 5/387 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLTableRowElement | createElement | 5/374 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLTableSectionElement | createElement | 5/371 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| HTMLUListElement | createElement | 5/366 | attributeStyleMap, computedStyleMap, part, requestFullscreen, requestPointerLock |
| InputEvent | new | 5/32 | dataTransfer, getTargetRanges, isComposing, sourceCapabilities, which |
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
| SpeechGrammarList | new | 3/4 | addFromString, item, length |
| TaskController | new | 3/3 | abort, setPriority, signal |
| Text | createTextNode | 3/77 | assignedSlot, nextElementSibling, previousElementSibling |
| XPathEvaluator | new | 3/3 | createExpression, createNSResolver, evaluate |
| XRRay | new | 3/3 | direction, matrix, origin |
| Comment | new | 2/70 | nextElementSibling, previousElementSibling |
| CompositionEvent | new | 2/29 | sourceCapabilities, which |
| DocumentTimeline | new | 2/3 | currentTime, duration |
| FocusEvent | new | 2/28 | sourceCapabilities, which |
| HashChangeEvent | new | 2/24 | newURL, oldURL |
| Highlight | new | 2/2 | priority, type |
| TouchEvent | new | 2/35 | sourceCapabilities, which |
| UIEvent | new | 2/27 | sourceCapabilities, which |
| WebTransport | new | 2/19 | incomingBidirectionalStreams, incomingUnidirectionalStreams |
| BarcodeDetector | new | 1/1 | detect |
| CaptureController | new | 1/13 | setFocusBehavior |
| ClipboardEvent | new | 1/23 | clipboardData |
| DocumentFragment | new | 1/62 | childElementCount |
| File | new | 1/11 | webkitRelativePath |
| Headers | new | 1/6 | getSetCookie |
| ImageData | new | 1/5 | colorSpace |
| InputDeviceCapabilities | new | 1/2 | firesTouchEvents |
| MediaMetadata | new | 1/5 | chapterInfo |
| MediaSource | new | 1/18 | handle |
| MediaStream | new | 1/15 | getTrackById |
| MessageEvent | new | 1/28 | initMessageEvent |
| OfflineAudioContext | new | 1/37 | audioWorklet |
| Path2D | new | 1/11 | roundRect |
| PopStateEvent | new | 1/24 | hasUAVisualTransition |
| RTCError | new | 1/34 | httpRequestStatusCode |
| RTCSessionDescription | new | 1/3 | toJSON |
| SpeechSynthesisUtterance | new | 1/17 | voice |
| SubmitEvent | new | 1/23 | submitter |
| XMLHttpRequest | new | 1/36 | setPrivateToken |

## 原型级探测（实例造不出，成员未核实）

AnalyserNode, AnimationPlaybackEvent, AnimationTrigger, AudioBufferSourceNode, AudioData, AudioDecoder, AudioEncoder, AudioProcessingEvent, AudioWorkletNode, BeforeInstallPromptEvent, BiquadFilterNode, BlobEvent, ByteLengthQueuingStrategy, CSSKeywordValue, CSSMathClamp, CSSMathInvert, CSSMathMax, CSSMathMin, CSSMathNegate, CSSMathProduct, CSSMathSum, CSSMatrixComponent, CSSPerspective, CSSRotate, CSSScale, CSSSkew, CSSSkewX, CSSSkewY, CSSTransformValue, CSSTranslate, CSSUnitValue, CSSUnparsedValue, CSSVariableReferenceValue, ChannelMergerNode, ChannelSplitterNode, CharacterBoundsUpdateEvent, ClipboardItem, CommandEvent, CompressionStream, ConstantSourceNode, ConvolverNode, CookieChangeEvent, CountQueuingStrategy, DecompressionStream, DelayNode, DeviceMotionEvent, DeviceOrientationEvent, DocumentPictureInPictureEvent, DragEvent, DynamicsCompressorNode, EncodedAudioChunk, EncodedVideoChunk, FederatedCredential, FencedFrameConfig, FontFaceSetLoadEvent, FormDataEvent, GPUInternalError, GPUOutOfMemoryError, GPUPipelineError, GPUUncapturedErrorEvent, GPUValidationError, GainNode, GamepadEvent, HIDConnectionEvent, HIDInputReportEvent, HTMLCameraElement, HTMLFencedFrameElement, HTMLGeolocationElement, HTMLHtmlElement, HTMLMicrophoneElement, HTMLSelectedContentElement, HTMLUserMediaElement, IDBVersionChangeEvent, IIRFilterNode, ImageCapture, ImageDecoder, KeyframeEffect, MIDIConnectionEvent, MIDIMessageEvent, MediaElementAudioSourceNode, MediaEncryptedEvent, MediaKeyMessageEvent, MediaQueryListEvent, MediaRecorder, MediaStreamAudioDestinationNode, MediaStreamAudioSourceNode, MediaStreamTrackEvent, NavigateEvent, NavigationCurrentEntryChangeEvent, Observable, OfflineAudioCompletionEvent, OscillatorNode, OverconstrainedError, PageRevealEvent, PageSwapEvent, PageTransitionEvent, PannerNode, PasswordCredential, PaymentMethodChangeEvent, PaymentRequest, PaymentRequestUpdateEvent, PerformanceObserver, PictureInPictureEvent, PresentationConnectionAvailableEvent, PresentationConnectionCloseEvent, PresentationRequest, PressureObserver, Profiler, PromiseRejectionEvent, RTCDTMFToneChangeEvent, RTCDataChannelEvent, RTCEncodedAudioFrame, RTCEncodedVideoFrame, RTCErrorEvent, RTCIceTransport, ReadableStreamBYOBReader, ReadableStreamDefaultReader, ReportingObserver, SecurityPolicyViolationEvent, SensorErrorEvent, SnapEvent, SpeechRecognitionErrorEvent, SpeechRecognitionEvent, SpeechRecognitionPhrase, SpeechSynthesisErrorEvent, SpeechSynthesisEvent, StaticRange, StereoPannerNode, TaskPriorityChangeEvent, TextFormatUpdateEvent, TextUpdateEvent, TrackEvent, USBAlternateInterface, USBConfiguration, USBConnectionEvent, USBEndpoint, USBInTransferResult, USBInterface, USBIsochronousInTransferPacket, USBIsochronousInTransferResult, USBIsochronousOutTransferPacket, USBIsochronousOutTransferResult, USBOutTransferResult, VTTCue, VideoDecoder, VideoEncoder, WaveShaperNode, WebGLContextEvent, WebSocket, WindowControlsOverlayGeometryChangeEvent, WritableStreamDefaultWriter, XRInputSourceEvent, XRInputSourcesChangeEvent, XRLayerEvent, XRReferenceSpaceEvent, XRSessionEvent, XRVisibilityMaskChangeEvent, XRWebGLBinding, XRWebGLLayer
