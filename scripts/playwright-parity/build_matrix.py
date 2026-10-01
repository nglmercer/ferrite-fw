import pathlib,re,json,collections
import argparse,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2]
DOCS=ROOT/'docs/e2e'
parser=argparse.ArgumentParser(description="Regenerate the pinned Playwright member inventory and source links")
parser.add_argument('--upstream', type=pathlib.Path, default=pathlib.Path.home()/'.cache/ferrite-playwright-audit/upstream')
parser.add_argument('--fetch', action='store_true', help='Download pinned API documents listed in sources.json')
args=parser.parse_args()
P=args.upstream
manifest=json.loads((pathlib.Path(__file__).parent/'sources.json').read_text())
if args.fetch:
 P.mkdir(parents=True,exist_ok=True)
 for name,path in manifest.items():
  with urllib.request.urlopen('https://raw.githubusercontent.com/microsoft/playwright/v1.63.0/'+path, timeout=30) as response:
   (P/name).write_bytes(response.read())
missing=[name for name in manifest if not (P/name).is_file()]
if missing:parser.error('Missing upstream documents: '+', '.join(missing)+'; use --fetch or --upstream PATH')
def langs(s):
 m=re.search(r'^\* langs:([^\n]*)',s,re.M)
 return not m or not m[1].strip() or 'js' in [x.strip() for x in m[1].split(',')]
classes={}
for path in sorted(P/name for name in manifest):
 s=path.read_text();heads=list(re.finditer(r'^## (?:(?:optional|async) )*(method|property|event): ([^\n]+)',s,re.M));cm=re.search(r'^# class: (\w+)',s,re.M)
 if not cm or not langs(s[:heads[0].start()] if heads else s):continue
 entries=[]
 for i,m in enumerate(heads):
  block=s[m.end():heads[i+1].start() if i+1<len(heads) else len(s)]
  meta=re.split(r'\n### ',block,maxsplit=1)[0]
  if not langs(meta):continue
  name=m[2].split(' = ')[0].split('#')[0]
  if any(x['name']==name and x['kind']==m[1] for x in entries):continue
  entries.append(dict(kind=m[1],name=name,deprecated='* deprecated:' in meta,source_file=path.name))
 classes[cm[1]]=entries
# Locate actual public functions and fields; mappings cannot point to invented APIs.
refs={}
files=list((ROOT/'crates/ferrite-e2e/src').glob('*.rs'))+[ROOT/'crates/ferrite-config/src/lib.rs']
for file in files:
 obj=''
 for n,line in enumerate(file.read_text().splitlines(),1):
  im=re.match(r'impl(?:<[^>]*>)?\s+(\w+)',line)
  st=re.match(r'pub (?:struct|enum|trait) (\w+)',line)
  if im:obj=im[1]
  if st:obj=st[1];refs[obj]=(file.relative_to(ROOT).as_posix(),n)
  fun=re.match(r'    pub (?:async )?fn (\w+)',line)
  if obj=='Reporter':fun=fun or re.match(r'    fn (\w+)',line)
  field=re.match(r'    pub (\w+):',line)
  free=re.match(r'pub (?:async )?fn (\w+)',line)
  if fun or field:refs[obj+'.'+(fun or field)[1]]=(file.relative_to(ROOT).as_posix(),n)
  if free:refs[free[1]]=(file.relative_to(ROOT).as_posix(),n)
M={}
def put(c,m,target='',note='',status='Partial',kind=None,source=None):
 if target and source is None:source=target
 if source:
  assert source in refs,(c,m,target,source)
 M[(c,m,kind)]=dict(status=status,target=target,note=note,ref=refs.get(source))
def group(c,d,note='',status='Partial'):
 for m,t in d.items():put(c,m,t,note,status)
def no(c,m,note):put(c,m,'',note,'Missing')
selnote='Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower.'
sel={'getByTestId':'get_by_test_id','getByText':'get_by_text','getByRole':'get_by_role_with','getByLabel':'get_by_label','getByPlaceholder':'get_by_placeholder','getByAltText':'get_by_alt','getByTitle':'get_by_title','locator':'locator'}
for c in ['Page','Locator','Frame']:
 group(c,{m:c+'.'+t for m,t in sel.items()},selnote)
 for m in ['content','title','url']:
  if c!='Locator':put(c,m,c+'.'+m,'Basic document access.',status='Equivalent')
group('Playwright',{'chromium':'Browser.launch','firefox':'Browser.launch'},'Select BrowserKind with LaunchOptions; no BrowserType object or bundled browser installer.')
put('Playwright','devices','DeviceDescriptor','Seven metrics presets; no full device catalog or device user-agent metadata.')
put('Playwright','request','ApiClient','Standalone or context-linked HTTP client; smaller APIRequest option surface.')
put('Playwright','selectors','set_test_id_attribute','Only test-id configuration; no custom selector registration.')
put('Playwright','errors','E2eError','Rust error enum, with different variants and diagnostics.','Idiomatic')
no('Playwright','webkit','No WebKit backend; BrowserKind contains Chromium and Firefox only.')
group('BrowserType',{'launch':'Browser.launch','connectOverCDP':'Browser.connect','executablePath':'find_chromium','name':'BrowserKind.name'},'Stock-browser discovery/launch; launched stdout/stderr are continuously drained with a bounded startup stderr tail. No channels/installer or full Playwright connection options.')
no('BrowserType','connect','No Playwright-protocol remote connection; Browser.connect is Chromium CDP over a loopback debug port.')
group('Browser',{'browserType':'Browser.kind','close':'Browser.close','contexts':'Browser.contexts','isConnected':'Browser.is_connected','newContext':'Browser.new_context','version':'Browser.version'},'Similar lifecycle/introspection; option sets and connection/context ownership differ.')
put('Browser','newPage','Browser.new_page','Uses the shared default context; Playwright creates a new context for this convenience API.')
put('Browser','newBrowserCDPSession','Browser.cdp','Raw shared browser CDP connection; no independently detachable CDPSession.')
group('CDPSession',{'send':'CdpSession.send_with','event':'CdpSession.events','detach':'CdpSession.detach_with','close':'CdpSessionEvents.next'},'Independently owned Chromium Page target sessions; Rust streams/operation controls with explicit loss/disposal errors, no callback close event. Page/context/transport lifecycle checked; last-owner drop queues detach.','Partial')
put('BrowserContext','newCDPSession','Page.new_cdp_session_with','Independent flattened Chromium Page-target session; Page convenience API, no Frame target overload or BrowserContext factory.')
put('CDPSession','close','CdpSessionEvents.next','Streams settle explicitly on detach/owner loss; no on(close) callback event.')
ctx={m:'BrowserContext.'+t for m,t in {'addCookies':'add_cookies','addInitScript':'add_init_script','clearCookies':'clear_cookies','clearPermissions':'clear_permissions','close':'close','cookies':'cookies','grantPermissions':'grant_permissions','newPage':'new_page','pages':'pages','route':'route_with_handler','routeFromHAR':'route_from_har','setExtraHTTPHeaders':'set_extra_http_headers','setGeolocation':'set_geolocation','setHTTPCredentials':'set_http_credentials','setOffline':'set_offline','storageState':'storage_state','setStorageState':'load_storage_state','tracing':'start_tracing','unrouteAll':'unroute_all','unroute':'unroute'}.items()}
group('BrowserContext',ctx,'Context API exists, with fewer options and engine restrictions; see the feature audit.')
put('BrowserContext','storageState','BrowserContext.storage_state','Captures only the first open page and one origin; no Playwright origins[] format, IndexedDB or OPFS persistence.')
put('BrowserContext','setStorageState','BrowserContext.load_storage_state','Loads Ferrite single-origin JSON; no native Playwright storage-state file compatibility.')
put('BrowserContext','clock','Page.clock_install','Page-document-local clock; no context-wide clock object.')
put('BrowserContext','request','ApiClient','Available separately; context.request with shared browser cookies is absent.')
put('BrowserContext','tracing','BrowserContext.start_tracing','Custom JSON trace, no Trace Viewer-compatible archive, DOM snapshots or chunks.')
for m in ['setOffline','setExtraHTTPHeaders','setHTTPCredentials']:
 put('BrowserContext',m,ctx[m],'Chromium only; HTTP credentials hold one username/password pair, no origin list.')
put('BrowserContext','grantPermissions','BrowserContext.grant_permissions','No origin argument; Chromium grants broadly, Firefox grants after navigation for the current origin.')
put('BrowserContext','clearCookies','BrowserContext.clear_cookies','Clears all cookies; no name/domain/path filters.')
pg={'addInitScript':'add_init_script','bringToFront':'bring_to_front','isClosed':'is_closed','evaluate':'evaluate','evaluateHandle':'evaluate_handle','exposeFunction':'expose_function','goBack':'go_back','goForward':'go_forward','requestGC':'request_gc','goto':'goto_with_options','reload':'reload','setContent':'set_content','setDefaultTimeout':'set_timeout','setExtraHTTPHeaders':'set_extra_http_headers','setInputFiles':'set_input_files','setViewportSize':'set_viewport','viewportSize':'viewport_size','screenshot':'screenshot','pdf':'pdf','emulateMedia':'emulate_media','consoleMessages':'console_messages','requests':'requests','addLocatorHandler':'add_locator_handler_with','removeLocatorHandler':'remove_locator_handler','route':'route_with_handler','routeFromHAR':'route_from_har','unroute':'unroute','unrouteAll':'unroute_all','opener':'opener','waitForEvent':'wait_for_event','waitForFunction':'wait_for_function','waitForLoadState':'wait_for_load_state','waitForSelector':'wait_for_selector_with','waitForTimeout':'wait_for_timeout','waitForURL':'wait_for_url','waitForRequest':'wait_for_request','waitForResponse':'wait_for_response','ariaSnapshot':'aria_snapshot','ariaSnapshotJSON':'aria_snapshot_json','clock':'clock_install','keyboard':'press_key','mouse':'mouse_click','touchscreen':'touchscreen_tap','localStorage':'local_storage_get','sessionStorage':'session_storage_get','video':'start_video','screencast':'frames','frame':'frame_by_name','frames':'document_frames'}
group('Page',{m:'Page.'+t for m,t in pg.items()},'Similar operation, with a smaller option/result/event surface; see feature audit.')
for m in ['goto','goBack','goForward','reload']:
 put('Page',m,'Page.'+pg[m],'Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL.')
put('Page','evaluate','Page.evaluate','Expression string plus JSON deserialization; no separate argument/JSHandle/function bridge or arbitrary JS value serialization.')
put('Page','evaluateHandle','Page.evaluate_handle','Remote JSHandle supported; no ElementHandle conversion or separate evaluation argument.')
put('Page','exposeFunction','Page.expose_function','JSON callback installed in the current document only; must be re-exposed after navigation, unlike Playwright; no binding/source metadata.')
for m in ['ariaSnapshot','ariaSnapshotJSON']:
 put('Page',m,'Page.'+pg[m],'Approximate flat DOM list, limited to 200 elements and 80-character names; no full ARIA tree, mode/depth/boxes options.')
put('Page','clock','Page.clock_install','Fake clock is scoped to the current document and resets on navigation; semantic differences below.')
put('Page','addLocatorHandler','Page.add_locator_handler_with','Triggered by element count before DOM actions, not visibility before action/assertion retries; lacks noWaitAfter and dismissal waiting.')
put('Page','waitForURL','Page.wait_for_url','Substring match only; no exact/glob/regex/predicate or waitUntil option.')
for m in ['waitForRequest','waitForResponse']:
 put('Page',m,'Page.'+pg[m],'URL substring match; returns RecordedRequest rather than rich Request/Response; no predicate overload.')
put('Page','waitForFunction','Page.wait_for_function','Polling expression to completion returns unit; no JSHandle, argument or configurable polling mode.')
put('Page','waitForSelector','Page.wait_for_selector_with','Waits for requested state and returns Locator, not ElementHandle.')
put('Page','emulateMedia','Page.emulate_media_with','Chromium typed media/color scheme/reduced motion/forced colors/contrast updates and individual resets; legacy partial calls preserve other overrides. Firefox rejects changed profiles; no context/test binding.')
put('Page','requestGC','Page.request_gc','Chromium only; Firefox returns an unsupported error.')
put('Page','pdf','Page.pdf_with','Validated Chromium PdfOptions: typed format/custom px/in/cm/mm dimensions, margins, scale/background/landscape, header/footer HTML, CSS size, one-based ranges, tagged/outline flags and a shared font/print clock. Basic pdf() remains native on both engines; new options explicitly reject Firefox. Defaults use zero margins, unlike legacy native defaults. Exact metric conversion differs slightly from pinned rounded JS factors; PDF size quantization/native print CSS remain visible.')
put('Page','request','ApiClient','Standalone client only; no page.request with shared browser cookies.')
for m,keys in {'addScriptTag':['add_script_tag_url','add_script_tag_content'],'addStyleTag':['add_style_tag_url','add_style_tag_content']}.items():
 put('Page',m,'Page.'+keys[0],'URL/content helpers return unit; no ElementHandle return, file/type options; other helper: Page.'+keys[1])
put('Page','mainFrame','Page.document_frames','Enumerate frames and choose the root manually; no dedicated main_frame API.')
# Legacy selector-based Page/Frame methods can use a locator without adding a facade method.
legacy={'check':'check','click':'click','dblclick':'dblclick','dispatchEvent':'dispatch_event','fill':'fill','focus':'focus','getAttribute':'attribute','hover':'hover','innerHTML':'inner_html','innerText':'text','inputValue':'input_value','isChecked':'is_checked','isDisabled':'is_disabled','isEditable':'is_editable','isEnabled':'is_enabled','isHidden':'is_hidden','isVisible':'is_visible','press':'press','selectOption':'select_options','setChecked':'set_checked','tap':'tap','textContent':'text','type':'press_sequentially_with','uncheck':'uncheck'}
for c in ['Page','Frame']:
 for m,t in legacy.items():put(c,m,'Locator.'+t,c+'.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial.')
for c in ['Page','Frame']:put(c,'dragAndDrop','Locator.drag_to',c+'.locator(source).drag_to(target, steps); fewer options and frame-coordinate restrictions.')
put('Frame','setInputFiles','Locator.set_input_files','Use Frame.locator(selector); path uploads only and fewer options.')
for m,t in {'childFrames':'child_frames','parentFrame':'parent','isDetached':'is_detached','name':'name','url':'url','goto':'goto','evaluate':'evaluate'}.items():
 put('Frame',m,'Frame.'+t,'Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes.')
# Locator families.
ln={'all':'all','and':'and_','or':'or_','blur':'blur','boundingBox':'bounding_box','check':'check','clear':'clear','click':'click_with_options','count':'count','dblclick':'dblclick','dispatchEvent':'dispatch_event','dragTo':'drag_to','evaluate':'evaluate','fill':'fill','filter':'filter_with','first':'first','focus':'focus','getAttribute':'attribute','highlight':'highlight','hover':'hover','innerHTML':'inner_html','innerText':'text','inputValue':'input_value','isChecked':'is_checked','isDisabled':'is_disabled','isEditable':'is_editable','isEnabled':'is_enabled','isHidden':'is_hidden','isVisible':'is_visible','last':'last','nth':'nth','press':'press_with','pressSequentially':'press_sequentially_with','screenshot':'screenshot','scrollIntoViewIfNeeded':'scroll_into_view','selectOption':'select_options','selectText':'select_text','setChecked':'set_checked','setInputFiles':'set_input_files','tap':'tap','textContent':'text','type':'press_sequentially_with','uncheck':'uncheck','waitFor':'wait_for_state'}
group('Locator',{m:'Locator.'+t for m,t in ln.items()},'Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower.')
for m in ['count','all','nth','and','or']:put('Locator',m,'Locator.'+ln[m],'Equivalent basic collection/selection operation; selector limitations still apply.','Equivalent')
put('Locator','first','Locator.first','Narrows the resolved set to its first match, including count/all; selector semantics remain narrower.')
put('Locator','filter','Locator.filter_with','Relative has/hasNot/text and visibility filters; exact/regex matching through locator builders. Inner locators must share the document.')
put('Locator','dispatchEvent','Locator.dispatch_event','Always creates a bubbling CustomEvent with detail; not Playwright eventInit/type-specific event behavior.')
put('Locator','fill','Locator.fill','Retries visibility/enabled/editability, uses native input setters or contenteditable text plus input/change events; not all native input validation/event semantics.')
put('Locator','pressSequentially','Locator.press_sequentially_with','Trusted per-character key input with optional delay; fewer keyboard layout/modifier options.')
put('Locator','textContent','Locator.text','Returns trimmed text as String; differs from nullable, untrimmed textContent.')
put('Locator','innerText','Locator.text','Reads trimmed textContent, not rendered innerText.')
put('Locator','selectOption','Locator.select_options','Value/label/index matching exists; returns unit rather than selected values, with fewer options.')
put('Locator','evaluate','Locator.evaluate','Function string over the first element, JSON result; no explicit arg/JSHandle support.')
put('Locator','describe','Locator.describe','Optional label stored separately from DOM selectors; clone/decorators retain it, derived selectors clear it, and content-frame owner retains it. Labels appear once in Rust action/assertion errors, steps and owned traces; the actual pinned Playwright timeout omits the label.')
put('Locator','description','Locator.description','Returns Option<&str>; empty describe or clear_description removes the label. Actual pinned clone/chaining observations verified on both native engines.')
put('Locator','toString','Locator','Rust Display/to_string returns the label or raw selector; selector() always exposes the raw selector. No Playwright locator expression representation.','Idiomatic')
# Assertions: positive matchers plus not(), subset of assertion argument/options.
an={'not':'not','toBeAttached':'attached','toBeChecked':'checked','toBeDisabled':'disabled','toBeEditable':'editable','toBeEmpty':'empty','toBeEnabled':'enabled','toBeFocused':'focused','toBeHidden':'hidden','toBeInViewport':'in_viewport','toBeVisible':'visible','toContainClass':'contains_class','toContainText':'contains_text','toHaveAccessibleDescription':'accessible_description','toHaveAccessibleName':'accessible_name','toHaveAttribute':'attribute','toHaveCount':'count','toHaveCSS':'css','toHaveId':'id','toHaveJSProperty':'js_property','toHaveScreenshot':'screenshot_with','toHaveText':'text','toHaveValue':'value'}
group('LocatorAssertions',{m:'LocatorExpect.'+t for m,t in an.items()},'Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ.')
put('LocatorAssertions','toBeInViewport','LocatorExpect.in_viewport','Checks rectangle overlap only; no IntersectionObserver ratio option.')
for m in ['toHaveAccessibleName','toHaveAccessibleDescription']:put('LocatorAssertions',m,'LocatorExpect.'+an[m],'Approximate DOM computation; no full accessibility algorithm or regex matching.')
for m in ['toHaveClass','toHaveValues','toHaveRole','toHaveAccessibleErrorMessage','toMatchAriaSnapshot']:no('LocatorAssertions',m,'No dedicated matcher; use available state/getters and expect_poll for custom checks.')
group('PageAssertions',{'not':'PageExpect.not','toHaveTitle':'PageExpect.title','toHaveURL':'PageExpect.url_contains','toHaveScreenshot':'PageExpect.screenshot_with','toMatchAriaSnapshot':'PageExpect.aria_snapshot'},'Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms.')
put('PageAssertions','toHaveURL','PageExpect.url_contains','Substring-only check, not Playwright exact string/regex/predicate semantics.')
put('SnapshotAssertions','toMatchSnapshot','assert_snapshot_text','Text and PNG helper functions; no Playwright snapshot-path template/source update/changed mode or arbitrary binary snapshots.')
group('PlaywrightAssertions',{'expectLocator':'Locator.expect','expectPage':'Page.expect'},'Rust assertion builders with attempt-owned TestInfo.soft_asserts / AttemptSoftAsserts.run contextual steps and automatic mismatch failure. Operational/control errors propagate; no custom matcher registry or full expect configuration.')
for m in ['toBeOK','not']:put('APIResponseAssertions',m,'ApiResponse.ok','Use assert!(response.ok()) or its negation; no dedicated retrying assertion object.','Idiomatic')
put('PlaywrightAssertions','expectAPIResponse','ApiResponse.ok','Use native Rust assertions over the standalone response.','Idiomatic')
put('PlaywrightAssertions','expectGeneric','','Use native Rust assertions and explicit pattern/container checks; no Playwright expect matcher library.','Idiomatic')
for e in classes.get('GenericAssertions',[]):
 m=e['name'].split('.',1)[1]
 put('GenericAssertions',m,'','Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object.','Idiomatic')
# Low-level inputs and handles.
group('Keyboard',{'down':'Page.key_down','insertText':'Page.insert_text','press':'Page.press_key_with','up':'Page.key_up','type':'Locator.press_sequentially_with'},'Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults.')
group('Mouse',{'click':'Page.mouse_click_with','dblclick':'Page.mouse_click','down':'Page.mouse_down','move':'Page.mouse_move','up':'Page.mouse_up','wheel':'Page.mouse_wheel'},'Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options.')
put('Touchscreen','tap','Page.touchscreen_tap','Coordinate tap available; no separate Touchscreen object.')
group('JSHandle',{'dispose':'JSHandle.dispose','evaluate':'JSHandle.evaluate','getProperty':'JSHandle.get_property','jsonValue':'JSHandle.json_value'},'Remote references exist; expression-string/JSON bridge has fewer value and argument types.')
for m,t in {'install':'clock_install','runFor':'clock_run_for','fastForward':'clock_fast_forward','setFixedTime':'clock_set_fixed_time','setSystemTime':'clock_set_system_time','resume':'clock_resume','pauseAt':'clock_pause'}.items():
 put('Clock',m,'Page.'+t,'Manual document-local fake timers only; see clock semantic differences in feature audit.')
put('Clock','fastForward','Page.clock_fast_forward','Fires due timers at most once; document-local controller rather than context-wide.')
put('Clock','setFixedTime','Page.clock_set_fixed_time','Mutates the same timer clock; subsequent advances move Date too, rather than fixing Date independently.')
put('Clock','setSystemTime','Page.clock_set_system_time','Alias for set_fixed_time; changes the timer clock as well as Date.')
put('Clock','resume','Page.clock_resume','Only toggles a flag; it does not restart wall-time timer progression.')
put('Clock','pauseAt','Page.clock_pause','No pause-at-time argument or skip-to-time semantics; toggles paused flag only.')
# Network value objects.
put('APIRequest','newContext','ApiClient.new','Standalone client constructor; no options/configurable cookie storage.')
for m,t in {'delete':'delete','fetch':'fetch','get':'get','head':'head','patch':'patch_json','post':'post_json','put':'put_json'}.items():put('APIRequestContext',m,'ApiClient.'+t,'HTTP verb available; fetch_with adds payload/header/timeout/status, per-request redirect limits and reset-only retries. Manual redirect methods, replay, hop cookies and cancellation share one budget; client TLS options and legacy preemptive Basic defaults remain narrower.')
for m,t in {'body':'bytes','headers':'headers','headersArray':'headers','json':'json','ok':'ok','status':'status','text':'text'}.items():put('APIResponse',m,'ApiResponse.'+t,'Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8.',status='Equivalent' if m in ['body','json','ok','status'] else 'Partial')
group('Request',{'allHeaders':'RecordedRequest.headers','headers':'RecordedRequest.headers','headersArray':'RecordedRequest.headers','method':'RecordedRequest.method','postData':'RecordedRequest.post_data','url':'RecordedRequest.url'},'Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged.')
put('Request','postDataJSON','RecordedRequest.post_data','Caller parses the captured request text with serde_json; body_json() is RESPONSE JSON, not request postDataJSON.','Partial')
put('Request','postDataBuffer','RecordedRequest.post_data','Text capture only; no lossless binary request body API.')
put('Request','response','RecordedRequest.status','Captured response data are merged into the record; no separate live Response or request lifecycle object.')
put('Request','timing','RecordedRequest.duration_ms','Total elapsed time only; no DNS/connect/TLS/response timing breakdown.')
for m,t in {'allHeaders':'response_headers','headers':'response_headers','headersArray':'response_headers','body':'body','json':'body_json','text':'body_text','status':'status','statusText':'status_text','url':'url'}.items():put('Response',m,'RecordedRequest.'+t,'Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox.')
put('Response','ok','RecordedRequest.status','Check (200..300).contains(&record.status); no dedicated Response.ok() method.','Idiomatic')
group('Route',{'abort':'RouteAction.abort_with','continue':'RouteRule.continue_with','fallback':'RouteAction.fallback','fetch':'RouteInfo.fetch_with','fulfill':'RouteInfo.fulfill_with','request':'RouteInfo'},'Rule/action model with returned decisions; native intercepted-request URL rewriting and response-stage modification are Chromium-only. Out-of-band HTTP fetch and prepared fulfillment work on both engines.')
put('Route','fetch','RouteInfo.fetch_with','Context-linked cookies/TLS/proxy/auth, live page deadlines, caller cancellation, method/header/raw/JSON and HTTP(S) URL overrides, redirect limits and reset-only retries. Relative URLs use the context base URL. Firefox original body bytes are unavailable and require an override; Chromium lossless binary capture is bounded. No automatic compression decoding or arbitrary streams.')
put('Route','fulfill','RouteInfo.fulfill_with','Owned API response inheritance, status/header/binary/JSON/file overrides, duplicate headers and request-origin CORS preparation. Pinned content-type/file/body/content-length precedence; final status validation, regular-file paths and explicit Rust preparation deadline/cancellation. Returned RouteAction rather than live-route method; legacy static helpers keep their contracts.')
# Downloads, dialogs, logs, trace/video/storage values.
group('Download',{'delete':'Download.delete','path':'Download.path','saveAs':'Download.save_as','suggestedFilename':'Download.suggested_filename','url':'Download.url','failure':'Download.failure'},'Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path.')
put('Download','cancel','Page.cancel_downloads','Cancels page-tracked downloads on Chromium; no per-Download.cancel method.')
for m,t in {'accept':'DialogDecision.accept_with','dismiss':'DialogDecision.dismiss','message':'DialogInfo.message','type':'DialogInfo.dialog_type'}.items():put('Dialog',m,t,'Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page.')
group('ConsoleMessage',{'text':'ConsoleMessage.text','type':'ConsoleMessage.kind'},'String previews only; no JSHandle arguments, source location, timestamp or page/worker ownership.')
put('Tracing','start','BrowserContext.start_tracing','Custom JSON actions/logs/requests; optional screenshots at Page.step only, no DOM/ARIA/source snapshots.')
put('Tracing','stop','BrowserContext.stop_tracing','Writes JSON, not a Trace Viewer-compatible zip archive.')
put('Tracing','startHar','Page.start_request_capture','Page capture + save_har_with; no Tracing.startHar API or full browser/API-request tracing.')
put('Tracing','stopHar','Page.save_har_with','HAR exporter; no Tracing.stopHar interface, update/rewrite mode or full timing/body coverage.')
put('Screencast','start','Page.start_video','Video recording/live frames available; fewer formats/options, no screencast overlay/action system.')
put('Screencast','stop','Page.stop_video','Explicit output path; Chromium assembles frames with ffmpeg, Firefox records natively.')
put('Video','path','TestResult.video','Runner records an optional artifact path, not Page.video()/Video object.')
put('Video','saveAs','Page.stop_video','Stop recording to a path; no independently awaitable Video handle.')
put('Video','delete','','Caller deletes artifact through Rust filesystem operations; no Video object.','Idiomatic')
group('WebStorage',{'getItem':'Page.local_storage_get','setItem':'Page.local_storage_set','removeItem':'Page.local_storage_remove','clear':'Page.local_storage_clear','items':'Page.local_storage_items'},'Page helper methods for local and session storage; no WebStorage object.')
put('Selectors','setTestIdAttribute','set_test_id_attribute','Process-global attribute override.','Equivalent')
# Events: only explicit PageEvent variants are supported. Context/browser emitters are absent.
for m,n in {'close':'Closed','console':'Console','dialog':'Dialog','download':'Download','popup':'Popup','request':'Request','response':'Response','webSocket':'WebSocket'}.items():
 put('Page',m,'Page.subscribe','PageEvent.'+n+' via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only.',kind='event')
for m,n in {'close':'Closed','frameReceived':'Received','frameSent':'Sent'}.items():put('WebSocket',m,'Page.subscribe','PageEvent::WebSocket direction '+n+'; Chromium-only observation without a WebSocket object.',kind='event')
socket_note='Chromium native socket_id scoped to Page, bounded history/payloads and explicit loss/truncation. Stock Firefox BiDi unsupported; no live socket/worker/route owner.'
for m,direction in {'close':'Closed','frameReceived':'Received','frameSent':'Sent','socketError':'Error'}.items():
 put('WebSocket',m,'Page.wait_for_websocket_event',socket_note+' Scoped future '+direction+' waits settle on native error/close, lag, disposal/disconnect and cancellation; historical retained native close may satisfy Closed.',kind='event')
put('WebSocket','isClosed','WebSocketSnapshot.is_closed',socket_note+' Some(true) requires native closure; lost observation returns None rather than inventing successful closure.')
put('WebSocket','url','WebSocketSnapshot.url',socket_note+' URL capped at 4 KiB with url_truncated; observing creation does not imply successful handshake.')
put('WebSocket','waitForEvent','Page.wait_for_websocket_event',socket_note+' Typed direction and native ID with shared operation options; no JS predicate/EventEmitter overloads.')
put('Page','webSocket','Page.wait_for_websocket',socket_note+' Full URL matcher for future creation; no relative URL resolution or successful-handshake promise.',kind='event')
put('WebSocketFrame','binary','WebSocketEvent.payload_bytes',socket_note+' Native opcode retained; complete CDP non-text base64 decoded only when untruncated. Text also has UTF-8 byte convenience; callers inspect opcode.')
put('WebSocketFrame','text','WebSocketEvent.payload',socket_note+' Text is UTF-8 only for opcode 1; binary/control frames retain native base64. Check opcode and payload_truncated before consumption.')
# Runner and configuration.
for m,t in {'afterAll':'after_all','afterEach':'after_each','beforeAll':'before_all','beforeEach':'before_each'}.items():put('Test',m,'Suite.'+t,'Nested suite hooks; beforeAll/afterAll once per worker/project, beforeEach/afterEach per attempt. Tokio worker state rather than process restarts; explicit typed ContextHook/WorkerHook requests with scope validation; no callback parameter inference.')
put('Test','(call)','test','Rust test closure; dynamic details/locks/options differ.')
put('Test','describe','Suite.tests','Nested identity, scoped hooks, inherited timeout/retry/context/tags; no serial/fully-parallel suite scheduling configuration.')
put('Test','expect','Locator.expect','Rust builders, expect_poll/expect_to_pass and options companions with validated intervals, messages, shared budgets, cancellation and scoped soft retries. Rust defaults/error/step semantics differ; no matcher registry/configure API.')
put('Test','extend','Runner.fixture_definition','Typed dependency graph, explicit lazy requests, automatic fixtures, test/worker scope and reverse dependency teardown. No named overrides or callback parameter inference. Built-in page/context/request/TestInfo dependencies and browser/WorkerInfo worker dependencies are available.')
for m,t in {'fail':'fail','fixme':'fixme','only':'only','skip':'skip','slow':'slow','setTimeout':'timeout'}.items():put('Test',m,'Test.'+t,'Static test builder setting; no runtime conditional/context metadata overloads.')
put('Test','info','TestContext.info','Provided through test_with_context; fewer live metadata fields and mutators.')
put('Test','step','Page.step','Named closure logged on Page; no structured step tree, boxing, timeout, subtitle/params or TestStepInfo.')
put('Test','use','Suite.context_options','Nested suite/test context inheritance; no general named fixture option overrides.')
for m in ['browser','browserName','context','page','request']:
 target={'browser':'Browser','browserName':'Browser.kind','context':'BrowserContext','page':'test','request':'ApiClient'}[m]
 put('Fixtures',m,target,'Manual Browser/Context/client setup or injected Page; runner injects a fresh context/page for every attempt.')
for m,t in {'attachments':'attachments','attach':'attach','file':'file','tags':'tags','line':'line','outputDir':'output_dir','project':'project','repeatEachIndex':'repeat_each_index','retry':'retry','timeout':'timeout','title':'title','workerIndex':'worker_index'}.items():put('TestInfo',m,'TestInfo.'+t,'Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ.')
for m,t in {'attachments':'attachments','annotations':'annotations','duration':'duration_ms','error':'error','status':'status','retry':'attempts'}.items():put('TestResult',m,'TestResult.'+t,'Aggregate result fields; attempts aggregates retries, not one TestResult per attempt; errors are strings. Live callbacks additionally receive per-attempt results; the final report aggregates retries.')
for m,t in {'annotations':'annotations','location':'file','repeatEachIndex':'repeat_each_index','retries':'retries','tags':'tags','timeout':'timeout','title':'name'}.items():
 if m in ['repeatEachIndex']:put('TestCase',m,'TestResult.'+t,'Result metadata only; no TestCase/Suite reporter tree.')
 else:put('TestCase',m,'Test.'+t,'Test definition field only; no TestCase/Suite reporter tree.')
group('Location',{'file':'TestInfo.file','line':'TestInfo.line'},'Rust track_caller metadata; no column.')
for c in ['TestInfoError','TestError']:
 put(c,'message','TestResult.error','String-formatted Rust error; no structured cause/stack/error-context object.')
for c in ['FullConfig','TestConfig']:
 d={'forbidOnly':'Runner.forbid_only','globalSetup':'Runner.global_setup','globalTeardown':'Runner.global_teardown','grep':'Runner.grep','grepInvert':'Runner.grep_invert','projects':'Runner.project','shard':'Runner.shard','workers':'Runner.workers','reporter':'E2eConfig.reporter','webServer':'E2eConfig.web_server','updateSnapshots':'E2eConfig.update_snapshots','outputDir':'E2eConfig.output_dir','repeatEach':'Runner.repeat_each','retries':'Runner.retries','timeout':'Runner.test_timeout','use':'ContextOptions','expect':'E2eConfig.expect_timeout_ms'}
 for m,t in d.items():put(c,m,t,'Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ.')
 put(c,'failOnFlakyTests','Runner.fail_on_flaky_tests','Opt-in aggregate failure preserves passed/flaky results and raw attempt statuses; CLI/config/effective snapshots wired. JUnit emits explicit policy violations, unlike upstream zero-failure XML.')
 put(c,'forbidOnly','Runner.forbid_only','Shared config/CLI and CI protection audit registered test/suite focus before filters/shards, including skipped/fixme/expected-failure descendants. Stricter than pinned upstream grep behavior.')
 put(c,'expect','E2eConfig.expect_timeout_ms','Default assertion retry window is applied to attempt pages and exposed in effective configuration; no full project expectation options/defaults tree.')
for c in ['FullProject','TestProject']:
 for m,t in {'name':'Project.name','grep':'Project.grep','retries':'Project.retries','timeout':'Project.timeout'}.items():put(c,m,t,'Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use.')
for m,t in {'acceptDownloads':'accept_downloads','bypassCSP':'bypass_csp','deviceScaleFactor':'device_scale_factor','extraHTTPHeaders':'extra_http_headers','geolocation':'geolocation','hasTouch':'has_touch','httpCredentials':'http_credentials','ignoreHTTPSErrors':'ignore_https_errors','isMobile':'is_mobile','javaScriptEnabled':'java_script_enabled','locale':'locale','offline':'offline','permissions':'permissions','proxy':'proxy_server','storageState':'storage_state','timezoneId':'timezone_id','userAgent':'user_agent','viewport':'viewport','serviceWorkers':'service_workers'}.items():put('TestOptions',m,'ContextOptions.'+t,'Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply.')
for m,t in {'baseURL':'base_url','browserName':'browser','headless':'headless','screenshot':'screenshot','video':'video'}.items():put('TestOptions',m,'E2eConfig.'+t,'Runner/launch config field; no automatic named-fixture/project options equivalence.')
put('TestOptions','launchOptions','LaunchOptions','Launch struct subset; no full Playwright options, persistent profile or channels.')
put('TestOptions','contextOptions','ContextOptions','Runner/project defaults and suite/test context inheritance; no general named fixture overrides.')
put('TestOptions','testIdAttribute','set_test_id_attribute','Process-global setter, not project/test-specific fixture option.')
put('TestOptions','trace','BrowserContext.start_tracing','Manual custom JSON traces plus runner JSON; no trace mode policy or Trace Viewer compatibility.')
for m in ['colorScheme','reducedMotion']:put('TestOptions',m,'Page.emulate_media','Chromium page-level manual emulation; no context/test option binding.')
# Practical parity implementation updates (2026-09-29).
put('Page','close','Page.close','Closes the target and its owning convenience context through once-only background disposal; dropped waits do not abandon it and repeated calls await its result. No runBeforeUnload/reason options.',kind='method')
put('BrowserContext','browser','BrowserContext.browser','Option<Browser> upgrades the actual weak shared owner for explicit, convenience, default/persistent and attached contexts. Retrieved handles retain the process; None after its last owner drops. No Android/Electron contexts or JavaScript identity semantics.')
put('BrowserContext','isClosed','BrowserContext.is_closed','Tracks explicit context disposal, shared browser shutdown, last-owner drop and native transport loss. Enum event waits distinguish disconnect errors from observed native events; no full upstream emitter/reason surface.')
put('Browser','isConnected','Browser.is_connected','Shared owner shutdown and actual transport reader/writer state. Clones share one process/profile/context registry; closing any handle shuts down all. Remote attachment closes Ferrite without killing the source process.')
put('Locator','visible','Locator.visible','Lazy visibility filter reapplied when resolving; uses the shared DOM visibility approximation.')
put('Browser','newPage','Browser.new_page','Fresh owning context; closing the page disposes it, including its popups.')
put('BrowserType','launchPersistentContext','LaunchOptions.user_data_dir','Reusable Chromium/Firefox profile; obtain browser.default_context(). Dedicated contexts remain isolated from persistent storage.')
put('BrowserType','connectOverCDP','Browser.connect_over_cdp','Chromium HTTP or browser WebSocket endpoint; no Playwright remote protocol or headers/options surface.')
put('BrowserContext','request','BrowserContext.request','Context-linked HTTP client sharing cookies and inheriting headers, Basic auth, TLS, proxy and timeout settings at creation. Transport overrides use ApiClientOptions.')
put('Page','request','Page.request','HTTP client sharing owning-context cookies and inheriting its transport defaults; cancellation follows context disposal.')
put('BrowserContext','storageState','BrowserContext.storage_state','Playwright cookies/origins JSON, localStorage from live pages across origins; closed-origin inventory and IndexedDB/OPFS are deferred.')
put('BrowserContext','setStorageState','BrowserContext.load_storage_state','Playwright multi-origin state and legacy files; cookies restore before navigation and localStorage before app scripts. IndexedDB/OPFS not persisted.')
for m,t in {'setDefaultTimeout':'set_default_timeout','setDefaultNavigationTimeout':'set_default_navigation_timeout'}.items():put('BrowserContext',m,'BrowserContext.'+t,'Shared action/protocol defaults update existing and future pages; zero disables timeout, cancellation is independently supported.')
put('Page','context','Page.context','Owning context while registered; returns Option and becomes None after context disposal.')
put('Page','setDefaultNavigationTimeout','Page.set_navigation_timeout','Navigation default distinct from locator timeout.')
for m,t in {'clearConsoleMessages':'clear_console_messages','pageErrors':'page_errors','clearPageErrors':'clear_page_errors','coverage':'coverage','frameLocator':'frame_locator'}.items():put('Page',m,'Page.'+t,'Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape.')
put('Page','exposeFunction','Page.expose_function','Survives navigation using init scripts; polled Rust callbacks, no context bindings, frame-wide dispatch or async callbacks.')
put('Page','evaluate','Page.evaluate_with_arg','JSON-serializable arguments and results; no JSHandle argument or arbitrary JS value serialization.')
put('Page','waitForFunction','Page.wait_for_function','Awaits Promise predicates correctly; polling returns unit, with no JSHandle/polling-mode/argument options.')
for m,t in {'allTextContents':'all_text_contents','allInnerTexts':'all_inner_texts','textContent':'text_content','innerText':'inner_text','evaluateAll':'evaluate_all','hideHighlight':'hide_highlight','page':'page','ariaSnapshot':'aria_snapshot','ariaSnapshotJSON':'aria_snapshot_json','contentFrame':'content_frame','frameLocator':'frame_locator','waitForFunction':'wait_for_function'}.items():put('Locator',m,'Locator.'+t,'Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only.')
put('Locator','evaluate','Locator.evaluate_with_arg','Function with element and JSON argument; no JSHandle arguments or arbitrary JS result serialization.')
for m,t in {'toHaveClass':'class','toHaveValues':'values','toHaveRole':'role','toHaveAccessibleErrorMessage':'accessible_error_message','toMatchAriaSnapshot':'aria_snapshot'}.items():put('LocatorAssertions',m,'LocatorExpect.'+t,'Retrying dedicated matcher; ARIA snapshots use exact text and a DOM approximation, not full YAML matching.')
put('PageAssertions','toHaveURL','PageExpect.url','Exact relative/base URL assertion plus url_matches() and url_contains(); narrower options and regex syntax.')
for m,t in {'evaluateHandle':'evaluate_handle','getProperties':'get_properties'}.items():put('JSHandle',m,'JSHandle.'+t,'Retains remote results and enumerable own string-keyed properties; no ElementHandle conversion/JSHandle arguments.')
for m,t in {'fastForward':'clock_fast_forward','runFor':'clock_run_for','pauseAt':'clock_pause_at','resume':'clock_resume','setSystemTime':'clock_set_system_time','setFixedTime':'clock_set_fixed_time','install':'clock_install_at'}.items():put('Clock',m,'Page.'+t,'Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated.')
for m,t in {'startJSCoverage':'start_js_coverage_with','stopJSCoverage':'stop_js_coverage_with_options','startCSSCoverage':'start_css_coverage_with','stopCSSCoverage':'stop_css_coverage_with_options'}.items():put('Coverage',m,'Coverage.'+t,'Chromium native coverage with reset-on-navigation, JS anonymous-script reporting and optional source collection/status. Cached CSS sheets survive navigation with empty ranges when native counters vanish; JS reset=false cannot guarantee native persistence. Shared lifecycle/deadline/cancellation, dirty-domain restart and explicit lag/cap failures. Raw V8/CSS rule ranges, not flattened CSS used ranges; bounded source storage and Firefox unsupported.')
for m,t in {'frameLocator':'frame_locator','locator':'locator','owner':'owner','first':'first','last':'last','nth':'nth','getByRole':'get_by_role','getByText':'get_by_text','getByLabel':'get_by_label','getByTestId':'get_by_test_id','getByPlaceholder':'get_by_placeholder','getByAltText':'get_by_alt','getByTitle':'get_by_title'}.items():put('FrameLocator',m,'FrameLocator.'+t,'Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred.')
for m,t in {'url':'url','statusText':'status_text','dispose':'dispose'}.items():put('APIResponse',m,'ApiResponse.'+t,'Final response URL/status text and explicit body-buffer disposal; clones own their buffers.')
put('APIRequestContext','fetch','ApiClient.fetch_with','Generic method with exclusive JSON/form/multipart/raw payloads, per-request redirect limits, pre-header reset retries, hop cookies and 2xx/3xx status policy. One total deadline/cancellation; replayable owned bodies, client-scoped TLS opt-out, no automatic compression decoding.')
put('APIRequest','newContext','ApiClient.with_options','Cookie jar, base URL, headers, TLS, proxy, timeout, redirects and Basic auth; optional browser cookie link, no full Playwright options.')
put('TestInfo','outputPath','TestInfo.output_path','Attempt-specific artifact path; parent traversal and absolute paths rejected. No snapshot-path templates.')
put('Test','locks','Test.lock','Named sorted locks serialize matching tests within this runner; Tokio workers, no multi-process worker coordination.')
put('Assertions','toPass','expect_to_pass_with','Retry a local E2eResult assertion block with PollingOptions; only assertion mismatches retry, default timeout remains 5s and one outer step is recorded. Last completed screenshot mismatch images publish only on final outer failure; nested probes transfer or discard them. No custom matcher registry.')
for c in ['FullConfig','TestConfig']:put(c,'expect','E2eConfig.expect_timeout_ms','Consumed by CLI environment bridge and Runner; per-page/expect overrides supported.')
for c in ['FullProject','TestProject']:put(c,'use','Project.context_options','Project-specific context settings; optional browser/launch overrides, suite/test context inheritance, no named fixture overrides or project dependencies.')
put('TestOptions','browserName','Project.browser','Projects select Chromium/Firefox independently; no WebKit backend.')
put('TestOptions','launchOptions','Project.launch_options','Dedicated project launch settings; persistent profiles supported, no managed channels.')
put('TestOptions','contextOptions','Project.context_options','Isolated context per attempt, runner defaults and per-project overrides; no full named-fixture test.use model.')
put('Page','ariaSnapshot','Page.aria_snapshot','Structured role/name/state DOM approximation, including open shadow roots; no full ARIA/YAML matching, mode/depth/boxes options.')
put('Page','ariaSnapshotJSON','Page.aria_snapshot_json','Nested role/name/state DOM tree without name/node truncation; not the complete accessibility algorithm.')
for cls, target in [('Page', 'Page'), ('Locator', 'Locator')]:
 for member, method in [('ariaSnapshot', 'aria_snapshot_with'), ('ariaSnapshotJSON', 'aria_snapshot_json_with')]:
  put(cls, member, target+'.'+method, 'Opt-in bounded DOM approximation: role depth, rounded frame-local boxes, optional supported state, explicit traversal/name budgets and deterministic safety markers. Legacy methods retain unbounded output. Native name/layout work is not preemptible; text fragments, full accessible names, AI modes and YAML syntax differ.')
for cls, target in [('PageAssertions', 'PageExpect'), ('LocatorAssertions', 'LocatorExpect')]:
 put(cls, 'toMatchAriaSnapshot', target+'.aria_snapshot_with', 'Retrying exact Ferrite text with bounded capture options; DOM approximation and truncation markers, no upstream YAML patterns or AI modes.')

put('Page','addLocatorHandler','Page.add_locator_handler_with','Visibility-based overlay handlers run before actions and state/custom assertions; no full dismissal/noWaitAfter semantics.')
put('Page','waitForFunction','Page.wait_for_function_handle','JSON argument, native animation-frame/interval polling and retained truthy result; JSON helper supports frames. Frame remote handles and arbitrary argument serialization remain unsupported. Legacy expression helper returns unit.')
put('Page','mainFrame','Page.main_frame','Dedicated asynchronous native root lookup; closed/disconnected pages fail, no fabricated root. Selector-free OOPIF traversal remains deferred.')
put('Page','frame','Page.frame_by_url_matching','Exact/contains/glob/regex or URL predicate snapshot lookup, plus existing name/substring helpers; relative matchers resolve base URL. Native Firefox frame names remain empty.')
put('Frame','url','Frame.current_url','current_url() reads live URL; url() retains lookup snapshot. Native realm/tree errors propagate after detach.')
put('Frame','isDetached','Frame.is_detached','Asynchronous native tree identity check; explicit page closure is detached, disconnection errors propagate. No replacement retargeting.')
put('Frame','name','Frame.name','Native lookup name snapshot; Chromium reports names, Firefox metadata is empty. No inferred name.')
put('Download','createReadStream','Download.create_read_stream','Tokio AsyncRead/AsyncSeek file for a completed successful download; options bound open only, caller can wrap reads in CancellationToken.run. Active native streams remain unsupported.')
put('Download','page','Download.page_id','Owning native page identity without retaining a live Page; None for hand-built completed paths. No upstream live page object.')
put('Download','delete','Download.delete','Completed file deletion is idempotent only for NotFound; other filesystem errors propagate. Active downloads are not represented.')
# Reliability, API authentication state, frame helpers, limits and context events.
for m,t in {'page':'page','setContent':'set_content','waitForFunction':'wait_for_function','waitForURL':'wait_for_url','waitForLoadState':'wait_for_load_state','waitForSelector':'wait_for_selector'}.items():
 put('Frame',m,'Frame.'+t,'Frame-scoped counterpart; unit-returning waits, fewer predicate/options modes; frame NetworkIdle remains unsupported. current_url() reads navigation updates.')
put('Frame','waitForFunction','Frame.wait_for_function_value','JSON arguments, native animation-frame/interval polling and captured JSON results; frame remote handles unsupported. Legacy expression helper returns unit.')
for m,t in {'storageState':'storage_state','setStorageState':'apply_storage_state','dispose':'dispose'}.items():
 put('APIRequestContext',m,'ApiClient.'+t,'Enumerable cookies with domain/path/expiry/HttpOnly/Secure/SameSite state import/export; origin data retained, IndexedDB deferred. Disposal cancels clones; returned Rust response buffers remain independently owned.')
put('APIRequest','newContext','ApiClient.with_options','Base URL, headers, client TLS/proxy, timeout, manual redirect limits, imported storage state and origin-scoped Basic auth. Legacy preemptive default and explicit-header precedence; Unauthorized selects challenge-only behavior. URL-normalized credential origins differ from raw upstream strings; no full options surface.')
put('BrowserContext','waitForEvent','BrowserContext.wait_for_event','Context-wide page/popup, console/error, network, download and close events with source page identity; enum-based filtering, no listener callback API or rich live Request/WebError objects.')
for m,t in {'page':'Page','console':'Console','weberror':'PageError','request':'Request','response':'Response','close':'Closed'}.items():
 put('BrowserContext',m,'BrowserContext.subscribe','ContextEventKind::'+t+'; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model.','Partial','event')
for c in ['FullConfig','TestConfig']:
 for m,t in {'globalTimeout':'global_timeout_ms','maxFailures':'max_failures'}.items():
  put(c,m,'E2eConfig.'+t,'Consumed by Runner and CLI; global cancellation with bounded teardown and final unexpected-failure scheduling limit. Active workers finish on maxFailures; no process-worker orchestration.')
put('BrowserContext','close','BrowserContext.close','Once-only background cleanup survives dropped waits and repeated calls await completion. Live disposal errors are aggregated; an already-lost transport permits idempotent local cleanup without native release confirmation. No reason option.',kind='method')
# Scoped fixtures, suites and native network lifecycle events.
for c in ['Page', 'BrowserContext']:
 for m,t in {'requestFinished':'RequestFinished','requestFailed':'RequestFailed'}.items():
  put(c,m,c+'.subscribe',t+' enum events on Chromium/Firefox with request IDs and method/URL; failed requests carry transport error text. Context events include page identity. No rich live Request object graph.','Partial','event')
for m,t in {'describe.only':'only','describe.skip':'skip','describe.fixme':'fixme'}.items():
 put('Test',m,'Suite.'+t,'Applies focus/skip/fixme to descendants; Rust builders, no JavaScript describe callbacks.')
put('Test','describe.configure','Suite.timeout','Suite timeout/retry/context settings inherited by descendants; no serial/fully-parallel execution mode configuration.')
# Context-aware fixtures, runtime controls and live reporter callbacks.
for m,t in {'fail':'fail','skip':'skip','slow':'slow','setTimeout':'set_timeout','annotations':'annotations'}.items():
 put('TestInfo',m,'TestInfo.'+t,'Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity.')
for m,t in {'fail':'fail','skip':'skip','slow':'slow','setTimeout':'set_timeout'}.items():
 put('Test',m,'TestInfo.'+t,'Static Test builders plus runtime TestInfo controls; use Rust conditionals and skip(reason)? for immediate closure exit. No JavaScript overload inference.')
for m,t in {'beforeEach':'before_each_with_context','afterEach':'after_each_with_context','beforeAll':'before_all_with_context','afterAll':'after_all_with_context'}.items():
 put('Test',m,'Suite.'+t,'ContextHook explicitly declares lazy fixture roots; WorkerHook permits only worker roots. Nested lifecycle ordering; no process workers or callback parameter inference.')
put('Test','extend','Runner.fixture_definition','Typed lazy built-in/user dependencies, test/worker scopes and reverse teardown. Separate Fixture.setup_timeout/teardown_timeout limits are capped by enclosing clocks; cleanup shares one scope budget, unlike upstream separate fixture accounting. No named overrides or callback parameter inference.')
for m,t in {'page':'page','context':'context','request':'request'}.items():
 put('Fixtures',m,'TestContext.'+t,'Fresh per-attempt resource, usable as a typed fixture dependency. request is isolated from browser cookies; context.request() shares cookies.')
for m,t in {'onBegin':'on_begin','onEnd':'on_end','onError':'on_error','onTestBegin':'on_test_begin','onTestEnd':'on_test_end','onStepBegin':'on_step_begin','onStepEnd':'on_step_end'}.items():
 put('Reporter',m,'Reporter.'+t,'Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture.')
for member, method in [('onEnd', 'on_end'), ('onTestEnd', 'on_test_end')]:
 put('Reporter', member, 'Reporter.'+method, 'Live synchronous callbacks retain attempt-specific steps, console and optional bounded data-only network summaries. Aggregate reports add inline search/status/project filters and pagination with portable artifact links. No Suite/TestCase graph, asynchronous end/status override, worker stdout or Trace Viewer archive.')
for m,t in {'title':'title','duration':'duration_ms'}.items():put('TestStep',m,'StepInfo.'+t,'Named Page.step live event metadata; no automatic action tree, parent hierarchy or structured error/source data.')
put('WorkerInfo','workerIndex','WorkerInfo.worker_index','Logical Tokio worker index, not a process identity.')
put('WorkerInfo','project','WorkerInfo.project','Optional project name only, not a resolved FullProject object.')
# Structured user steps, hook outcomes, and retained retry history.
put('Test','step','Page.step_with','Nested controlled steps with local timeout, skip, annotations and title paths through Page.step_with; legacy step_result remains available. Automatic navigation/locator/assertion/hook/fixture scopes; no boxing or subtitle/params options.')
for m,t in {'status':'status','expectedStatus':'expected_status','errors':'errors'}.items():
 put('TestInfo',m,'TestInfo.'+t,'Live getters shared across metadata clones; raw outcome published before afterEach and updated after cleanup failures. status is None until a body outcome or soft mismatch; soft failures publish immediately with source/step/message metadata and survive successful bodies/cleanup. Attempts seal against late writes; no JS stack/cause/snippet serialization.')
for m,t in {'title':'title','duration':'duration_ms','location':'location','error':'error','parent':'parent_id','startTime':'start_time_ms','steps':'steps','attachments':'attachments'}.items():
 put('TestStep',m,'StepInfo.'+t,'Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data.')
put('TestStepInfo','attach','TestInfo.attach','Attachments inside an awaited user step associate with that step and its attempt. Page.step_with passes a live StepContext; attachments still use TestInfo.attach and associate with the active scope. Detached Tokio tasks do not inherit parent scope.')
for m,t in {'file':'file','line':'line','column':'column'}.items():
 put('Location',m,'SourceLocation.'+t,'Rust caller location for user steps; test phase errors point to the test definition, with column zero when unknown.')
for c in ['TestInfoError','TestError']:
 put(c,'message','TestError.message','Structured Rust diagnostics carry message, code, phase and optional source; no JavaScript stack/cause/snippet object.')
put('TestError','location','TestError.location','Optional Rust source location; exact user-step call site, test definition for runner phase errors, not a JS throw-site location.')
for m,t in {'attachments':'attachments','annotations':'annotations','duration':'duration_ms','errors':'errors','startTime':'start_time_ms','status':'status','steps':'steps'}.items():
 put('TestResult',m,'AttemptResult.'+t,'Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees; soft_assertions retains contextual mismatch source and step paths with serde defaults. Different Rust schema, no stdout/stderr capture.')
for m,t in {'retry':'retry','workerIndex':'worker_index'}.items():
 put('TestResult',m,'AttemptInfo.'+t,'Per-attempt identity on AttemptResult.info; logical Tokio workers, not process-worker IDs.')
put('TestCase','results','TestResult.attempt_results','Full attempt history retained under the aggregate test result; no upstream TestCase/Suite graph.')
put('TestCase','outcome','TestResult.flaky','Recovered successful retries flagged flaky; aggregate TestStatus separates passed/failed/skipped/expected-failed. Rust fields, not the upstream outcome() enum.')
# Step control and automatic diagnostic scopes.
put('Test','step.skip','Page.step_with','StepOptions.skip records a skipped user step without constructing its closure; StepOutcome carries the reason. Rust option rather than a separate JS method.')
put('TestStepInfo','skip','StepContext.skip','Use step.skip(reason)? to abort only this controlled step. Shared clones cancel pending children; returns StepOutcome::Skipped, distinct from whole-test skip. Conditional skipping uses a Rust if statement.')
for m,t in {'annotations':'annotations','titlePath':'title_path'}.items():
 put('TestStepInfo',m,'StepContext.'+t,'Live getters on the context passed to Page.step_with; annotate adds source-aware metadata, title paths include file/test/ancestor steps. Completed records reject late annotations.')
for m,t in {'annotations':'annotations','category':'category','titlePath':'title_path'}.items():
 put('TestStep',m,'StepInfo.'+t,'Persisted annotations and full title path; categories user/action/assertion/hook/fixture differ from the upstream category vocabulary and not all Page/protocol operations are wrapped.')
# URL/network predicates, generated uploads and source-aware browser diagnostics.
for c in ['Page','Frame']:
 put(c,'waitForURL',c+'.wait_for_url_matching_with_options','Exact (relative to base URL), full-URL glob/regex or predicate with Commit/DOMContentLoaded/Load readiness in one navigation budget. Page NetworkIdle uses 500ms observed HTTP quiet; frames reject it. Legacy helpers retain substring/URL-only semantics. Zero timeout, cancellation and enclosing budgets supported; URLPattern absent.')
for m,t in {'waitForRequest':'wait_for_request_async','waitForResponse':'wait_for_response_async'}.items():
 put('Page',m,'Page.'+t,'Exact/glob/regex or sync/async predicates over RecordedRequest fields; request waits resolve at start, response waits at headers including already in-flight requests. Returns a metadata snapshot rather than rich live Request/Response/body objects. Legacy strings retain substring semantics; lag fails explicitly.')
for c,target in [('Page','Page.set_input_file_payloads'),('Locator','Locator.set_input_file_payloads'),('Frame','Locator.set_input_file_payloads')]:
 put(c,'setInputFiles',target,'FilePayload filename/MIME/bytes or existing path uploads; empty lists clear, multiple files require a multiple input, 64 MiB total cap. DOM File/DataTransfer injection on both engines; no native chooser/directory upload/options parity. Frame uses Frame.locator.')
for m,t in {'text':'text','type':'kind','location':'location','timestamp':'timestamp_ms','page':'page_id'}.items():
 put('ConsoleMessage',m,'ConsoleMessage.'+t,'Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. Bounded tagged native argument previews and structured optional errors retain data through context/popup/attempt/trace history; no live JSHandle argument or worker ownership parity.')
put('ConsoleMessage','args','ConsoleMessage.arguments','Owned native argument previews: tagged JSON/null, special primitives, CDP property previews or BiDi typed values and explicit unavailable/reference/truncation flags. 64 arguments/32 KiB; no live handles or complete object serialization. Retains native bigint spelling where pinned Playwright 1.63 returned an undefined handle.')
put('WebError','error','ConsoleMessage.error','Optional owned PageErrorInfo with native own name/message/stack, constructor class and supplied stack frames. Firefox exposes only text/frames; no complete live JavaScript Error/cause/property object.')
put('WebError','page','ConsoleMessage.page_id','Native owning page identity retained in context error events and histories, without a live Page/worker object. Unknown fields remain optional.')
put('WebError','location','ConsoleMessage.location','Optional native source URL and zero-based line/column on context exception observations; absent source metadata stays unavailable rather than producing an always-present location object.')
# Audit current event forwarding and selector facades independently of old mappings.
for m,t in {'frameAttached':'FrameAttached','frameNavigated':'FrameNavigated','frameDetached':'FrameDetached','DOMContentLoaded':'DomContentLoaded','load':'Load','dialogClosed':'DialogClosed'}.items():
 put('Page',m,'Page.subscribe','PageEventKind::'+t+'; native frame/page identity, parent/URL/document metadata when supplied, same-document history events, child-first detach and main-document readiness. Owned metadata snapshots rather than live Frame/Dialog objects; optional Firefox subscriptions and frame-name absence remain explicit. Chromium current target only: OOPIF adoption is deferred and swap denotes session departure.','Partial','event')
for m,t in {'frameAttached':'FrameAttached','frameNavigated':'FrameNavigated','frameDetached':'FrameDetached','dialogClosed':'DialogClosed'}.items():
 put('BrowserContext',m,'BrowserContext.subscribe','ContextEventKind::'+t+' forwards native page/frame or dialog-close metadata exactly once, with source page identity, including ingress popup observations before adoption. Owned snapshots rather than live upstream objects; optional native fields, bounded popup capture and Firefox subscription limits remain explicit. Chromium OOPIF sessions remain deferred.','Partial','event')
put('Page','waitForEvent','Page.wait_for_event','Enum-based page observations, including native frame/document readiness and dialog closure; shared deadlines/cancellation/disposal/transport loss. New Firefox events are capability-probed, missing navigation/closure support fails explicitly. Closure payloads are native snapshots rather than live Dialog objects; callback listener parity remains narrower.')
put('Locator','dispatchEvent','Locator.dispatch_event_with','Typed synthetic DOM constructors with JSON event-specific initialization and bubbles/cancelable/composed flags; CustomEvent legacy helper retained. Auto input events follow the pinned Event constructor; InputEvent can be requested explicitly. No live handle arguments.')
for m,t in {'toHaveText':'text_with','toContainText':'contains_texts_with','toHaveClass':'class_with','toContainClass':'contains_class_tokens','toHaveValues':'values_with','toBeChecked':'checked_with','toBeInViewport':'in_viewport_with','toHaveAccessibleName':'accessible_name_with','toHaveAccessibleDescription':'accessible_description_with','toHaveAccessibleErrorMessage':'accessible_error_message_with'}.items():
 put('LocatorAssertions',m,'LocatorExpect.'+t,'Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent.')
for m,t in {'dialog':'Dialog','download':'Download','pageClose':'PageClose'}.items():
 put('BrowserContext',m,'BrowserContext.subscribe','ContextEventKind::'+t+' forwards the existing page observation with source page ID; payloads and backend metadata remain narrower.','Partial','event')
for c in ['Page','Frame']:
 for m,t in {'innerText':'inner_text','textContent':'text_content'}.items():
  put(c,m,'Locator.'+t,c+'.locator(selector) followed by the distinct rendered/raw text getter; strict resolution and same-origin frame limitations remain.')
put('Locator','textContent','Locator.text_content','Untrimmed nullable DOM textContent with strict single-target resolution; no ElementHandle or full options surface.')
put('Locator','innerText','Locator.inner_text','Distinct rendered DOM innerText getter; strict resolution, nullable Rust result and same-origin lazy frame limitations remain.')
# Async and context callback lifecycle.
for c in ['Page','BrowserContext']:
 put(c,'exposeFunction',c+'.expose_function_async','Sync/async JSON callbacks in current/future same-origin documents; independent bounded dispatch, native preload ownership, duplicate-name errors and named removal. Rust errors/panics reject JS promises; cross-origin dispatch/handle arguments deferred.')
 put(c,'exposeBinding',c+'.expose_binding','Async JSON binding with owning context/page/native frame identity. Same-origin frame dispatch; native startup preloads and navigation/disposal cleanup. Cross-origin/OOPIF callers and handle arguments deferred.')
put('BrowserContext','clearCookies','BrowserContext.clear_cookies_with','ANDed exact/regex name/domain/path filters on native stores; empty filter clears all. Linked API requests refresh cookies; opaque partition key deletion fails explicitly on Chromium. Rust regex syntax and partition filter fields remain narrower.')
for m,t in {'click':'click_with_options','hover':'hover_with_options','check':'check_with_options','uncheck':'uncheck_with_options','setChecked':'set_checked_with_options','dragTo':'drag_to_with_options'}.items():
 put('Locator',m,'Locator.'+t,'Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower.')
# Typed native observations preserve the existing RecordedRequest APIs.
for m,t in {'method':'method','url':'url','postData':'post_data','postDataJSON':'post_data_json','failure':'failure','resourceType':'resource_type','isNavigationRequest':'is_navigation_request','redirectedFrom':'redirected_from','redirectedTo':'redirected_to','frame':'frame'}.items():
 put('Request',m,'Request.'+t,'Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph.')
for c in ['Request','Response']:
 for m,t in {'allHeaders':'headers_array','headers':'headers_array','headersArray':'headers_array','headerValue':'header_value'}.items():
  put(c,m,c+'.'+t,'Duplicate-preserving pairs and case-insensitive lookup; sync live getters rather than upstream await-for-all map APIs. Native extra events correlate by hop, with explicit completeness/truncation. Successfully acknowledged route-supplied pairs have an explicit source flag when events omit/fold them; same-URL hop identity preserved. Generic Firefox comma folding is not reversed.')
put('Request','response','Request.response','Returns an existing optional Response after headers; does not await future headers. Failed pre-header requests never fabricate a response. Use Page.wait_for_response_handle for a header wait.')
put('Request','existingResponse','Request.response','Existing optional Response after native headers; no full worker/OOPIF graph.')
for m,t in {'status':'status','statusText':'status_text','url':'url','ok':'ok','request':'request','frame':'frame','headerValues':'header_values'}.items():
 put('Response',m,'Response.'+t,'Typed native response metadata with owning request/page; headers may be natively folded, frame is async optional lookup, and fields remain bounded. Response bodies stay separate in legacy capture; no worker/security/full timing graph.')
put('Response','finished','Response.finished_with_options','Native completion independent of body capture, including Firefox; also settles associated fulfillment acknowledgements. HTTP errors finish successfully; transport failures return E2eError::Network. Shared defaults, zero/enclosing deadlines, cancellation, disposal and disconnect bounded; already terminal metadata survives page close.')
put('APIResponse','headersArray','ApiResponse.headers_array','Duplicate-preserving serialized HttpHeader entries with case-insensitive header_value/header_values companions; native binary route fulfillment and duplicate Set-Cookie/header forwarding verified on both engines. Legacy header() returns the first value.')
put('Page','requests','Page.network_requests','Typed metadata observed since page creation, capped at 4,096 hops/16 MiB; legacy Page.requests remains capture-based RecordedRequest snapshots. Eviction/truncation is explicit and bodies are separate.')
for m,t in {'waitForRequest':'wait_for_request_handle','waitForResponse':'wait_for_response_handle'}.items():
 put('Page',m,'Page.'+t,'Shared exact/base URL/glob/regex matchers and typed predicate companions, returning live per-hop Request/Response at start/headers. Existing snapshot and async predicate waits remain available. Cancellation/lag/disposal handled; no body capture or URLPattern implied.')
for m in ['request','response','requestFinished','requestFailed']:
 put('Page',m,'Page.subscribe_network','NetworkEvent Request/Response/Finished/Failed carries live per-hop handles; legacy Page.subscribe keeps existing payloads. 256-event channel, explicit lag errors and lifecycle/disconnection cancellation; context forwarding retains its existing enum snapshots.','Partial','event')
# Fill every remaining upstream member explicitly as absent, with class-specific explanations.
put('PageAssertions','toHaveURL','PageExpect.url_matching','Shared exact/base URL, glob, Rust regex or url_where predicate; retrying negation and cancellation. Legacy assertion helpers retain string contracts; explicit globs are a Rust extension, and URLPattern/case option parity remains absent.')
for c in ['Page','BrowserContext']:
 for m,t in {'route':'route_matching','unroute':'unroute_matching','routeFromHAR':'route_from_har'}.items():
  put(c,m,c+'.'+t,'Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately.')
for c in ['Page','BrowserContext']:
 for m,t in {'unrouteAll':'unroute_all_with','unroute':'unroute_matching_with'}.items():
  put(c,m,c+'.'+t,'Pattern/shared-matcher/all removal with default/wait/ignore-errors and explicit Rust Cancel. Default/ignore-errors release requests while callbacks settle and discard late decisions; wait preserves native decisions within shared deadlines. Independent bounded dispatch, atomic invocation limits including fallback, disposal/retry/disconnect cleanup verified on Chromium/Firefox. Rust first-registration/page priority and no callback-identity removal remain differences.')
# B09: capture options remain a partial rendering/engine surface.
put('Page','screenshot','Page.screenshot','Validated viewport/document clips, full-page masks/colors and temporary styles, Device/Css output and shared timeout. Chromium transparent PNG and retained metrics/background; Firefox Css raster normalization and explicit transparency unavailability. Owned restoration/cancellation and visible cleanup errors; native finite finish/infinite cancel-resume, zero-rate preservation and late CSS listeners. Application-realm helpers, formats and root traversal remain narrower.')
put('Locator','screenshot','Locator.screenshot_with','Element-defined document capture with masks/colors/styles, scale/background/quality and shared timeout. Scrolls into view, preserves metrics and owns restoration. clip/full_page rejected; Firefox transparency unavailable and Css uses raster normalization; same-origin frame/open-root limits remain.')
# B10 draft: stable capture, path templates and attempt-owned diffs are implemented.
for cls,target in [('PageAssertions','PageExpect'),('LocatorAssertions','LocatorExpect')]:
 put(cls,'toHaveScreenshot',target+'.screenshot_with','Successive stable PNG captures within one assertion window, real downloadable-font readiness, native animation finish/cancel-resume and B09 capture options and typed operation errors. Missing/all/changed generation requires stability; negation needs a valid baseline. Runner/project/explicit path templates; attempt/step-owned expected/actual/diff copies, last-pair stability diagnostics, final-only completed outer-poll images and portable bundles. Comparison/read/Css/diff/staging jobs have two active slots and cooperative cancellation, 64-million-pixel decoding and 512-MiB encoded-input limits. Frozen expected bytes, staged foreground baseline replacement, non-overwriting Missing installation and preserved symlink aliases. Queued memory and opaque OS/codec phases are not hard bounded. Final failure diagnostics share one additional five-second budget under enclosing cancellation/deadlines, with staged non-overwriting foreground attachment publication and a 1024-name collision cap. Per-channel comparison, visual diff, update/default semantics and retained single-mismatch probe semantics differ.')
put('SnapshotAssertions','toMatchSnapshot','assert_snapshot_text','Text and PNG helpers with missing/changed/all/none update modes and explicit path templates/context, including TestInfo.snapshot_options. Matching changed baselines retained; PNG validation and bounded pixels. No source-update modes, arbitrary binary snapshots or full upstream naming/kind overloads.')
path_note='Validated explicit token templates, optional single-character prefixes and frozen runner root; project and assertion overrides win. Adds browserName; Rust lowercase slugs, canonical png/snap extensions, nested string names and legacy paths differ. Unknown tokens fail; no anonymous/ARIA/snapshotSuffix overloads or JS configDir/testDir discovery semantics.'
metadata_note='JSON-safe BTreeMap user metadata, frozen before hooks and exposed through config/project_config and owned WorkerInfo snapshots; project values replace the whole map or inherit the run map. Explicit empty maps survive reports/CLI/bundles; no upstream actualWorkers injection or mutable JS object identity.'
for c in ['FullConfig','TestConfig']:
 put(c,'metadata','ResolvedRunConfig.metadata' if c=='FullConfig' else 'E2eConfig.metadata',metadata_note)
 put(c,'reportSlowTests','ResolvedRunConfig.report_slow_tests' if c=='FullConfig' else 'Runner.report_slow_tests','Optional SlowTestOptions; strict threshold_ms and bounded max <=1000, zero disables, absent opts preserve legacy reports. Rust summarizes scheduled test/project/repeat result indices, not source files; retry total duration counted once. Upstream defaults on and max zero is unlimited.')
for c in ['FullProject','TestProject']:
 put(c,'metadata','ResolvedProjectConfig.metadata' if c=='FullProject' else 'Project.metadata',metadata_note)
put('TestConfig','name','Runner.run_name','Optional Rust run name retained through frozen configuration and reports; not upstream test-directory naming or a mutable resolved JS configuration.')
put('TestConfig','snapshotPathTemplate','Runner.snapshot_path_template',path_note+' Shared E2eConfig/CLI/env bridge included.')
put('TestConfig','preserveOutput','Runner.output_retention','Validated always/never/failures-only config/env/CLI bridge, compatible always default. Final per-attempt expectation classification preserves failed retries/interruption/cleanup failures. Verified owned-directory cleanup protects baselines/caller sources; Rust prunes removed source links and preserves portable copies rather than retaining upstream stale attachment paths.')
put('FullConfig','preserveOutput','ResolvedRunConfig.output_retention','Frozen typed owned-output retention policy. Cleanup follows live reporters and bundle export; safe/pruned report links and fail-closed ownership/export checks are documented in D03.')
put('TestProject','snapshotPathTemplate','Project.snapshot_path_template',path_note)
put('TestInfo','snapshotPath','TestInfo.snapshot_path',path_note+' Typed SnapshotKind and no filesystem creation; retries share identity.')
# B14: effective configuration snapshots follow the actual selected work and native owners.
config_note='Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility.'
for m,t in {'workers':'workers','retries':'retries','timeout':'timeout_ms','grep':'grep','grepInvert':'grep_invert','repeatEach':'repeat_each','outputDir':'output_dir','snapshotDir':'snapshot_dir','projects':'projects','shard':'shard','reporter':'reporter','globalTimeout':'global_timeout_ms','maxFailures':'max_failures','use':'context','updateSnapshots':'snapshot_update','forbidOnly':'forbid_only','expect':'expect_timeout_ms'}.items():
 put('FullConfig',m,'ResolvedRunConfig.'+t,config_note)
put('FullConfig','failOnFlakyTests','ResolvedRunConfig.fail_on_flaky_tests','Effective opt-in aggregate failure policy; original passed/flaky results and attempts retained. CLI/config/Runner wiring and explicit JUnit policy violations are documented in D01.')
put('FullConfig','forbidOnly','ResolvedRunConfig.forbid_only','Effective config/CLI/CI focus protection; registered test/suite inventory audited before filters/shards, including skipped descendants. Stricter than pinned upstream grep behavior.')
for m,t in {'grep':'grep','grepInvert':'grep_invert','repeatEach':'repeat_each','outputDir':'output_dir','snapshotDir':'snapshot_dir','projects':'projects','shard':'shard'}.items():
 put('TestConfig',m,'E2eConfig.'+t,'Shared TOML/JSON configuration, complete CLI child bridge and Runner builders. Legacy environment overrides are read by from_env; explicit builder values win at run resolution. Name/tag substring filters and Rust scheduling differ from upstream.')
for m,t in {'name':'name','grep':'grep','grepInvert':'grep_invert','retries':'retries','timeout':'timeout_ms','repeatEach':'repeat_each','outputDir':'output_dir','snapshotDir':'snapshot_dir','use':'context'}.items():
 put('FullProject',m,'ResolvedProjectConfig.'+t,config_note+' Context overrides replace the whole value; launch proxy/TLS inheritance is shared with native creation.')
for m,t in {'grepInvert':'grep_invert','repeatEach':'repeat_each','outputDir':'output_dir','snapshotDir':'snapshot_dir'}.items():
 put('TestProject',m,'Project.'+t,'Library Project plus shared E2eProjectConfig fields and CLI configuration bridge; project overrides precede global values. Repetition zero normalizes to one, paths are absolute at run resolution; dependency scheduling remains absent.')
put('TestInfo','config','TestInfo.config',config_note)
put('TestInfo','project','TestInfo.project_config','Read-only selected project defaults through project_config, with actual attempt context/retry/repetition and runtime timeout through settings(). Existing public project remains an optional name. No full upstream FullProject object or dependency graph.')
def default_note(c,e):
 if c.startswith('Android') or c in ['Electron','ElectronApplication']:return 'Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend.'
 if c=='ElementHandle':return 'No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics.'
 if c=='FrameLocator':return 'Same-origin lazy FrameLocator exists; cross-origin selector-free search and OOPIF traversal remain deferred.'
 if c in ['Reporter','Suite','TestCase','TestRun','TestStep','TestStepInfo','WorkerInfo']:return 'No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls.'
 if c in ['Coverage','Credentials','Debugger','BrowserServer','Worker','WebSocketRoute','FileChooser','Logger']:return 'No public '+c+' abstraction or matching feature API; raw protocol calls are not counted as an equivalent.'
 if e['kind']=='event':return 'No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants.'
 return 'No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity.'
rows=[]
for c,entries in sorted(classes.items()):
 for e in entries:
  m=e['name'].split('.',1)[1];v=M.get((c,m,e['kind']),M.get((c,m,None)))
  # Shared method/event names must not accidentally map an absent event to a method.
  if e['kind']=='event' and (c,m,e['kind']) not in M:v=None
  if not v:v=dict(status='Missing',target='',note=default_note(c,e),ref=None)
  row=dict(e,**v,cls=c);rows.append(row)
counts=collections.Counter(x['status'] for x in rows)
for m in ['body','text','json']:
 put('Response',m,'Response.'+m,'Original native Chromium response bytes with opt-in capture, 1 MiB per-body cap and bounded history. Distinct pending/uncaptured/empty/truncated/unavailable/failed states; text replaces invalid UTF-8, typed JSON reports parse diagnostics. Shared timeout/cancellation/disposal while pending; cached bytes survive close. Firefox unsupported; existing Page.response_body remains a separate refetch helper.')
for c in ['Page','BrowserContext']:
 put(c,'routeFromHAR',c+'.route_from_har','Shared URL filters scope installed routes and explicit abort/fallback misses, including empty HAR abort rules. Duplicate candidates match exact method/URL/body, then maximize matching headers with stable file-order ties. Bounded plain JSON load; binary and redirect fulfillment supported. Firefox POST bytes remain unavailable. Legacy default fallback differs from Playwright abort; ZIP/update/attached content, automatic context recording and service-worker interception are not implied. Page.save_har_with_options adds full/minimal, omit/embed and timing omission over bounded native observations.')
md=['# Playwright API member matrix','', 'Audit date: 2026-09-29. Baseline: [Playwright v1.63.0](https://github.com/microsoft/playwright/releases/tag/v1.63.0). Local target: current working tree of `ferrite-e2e`, after the practical-parity implementation.','', 'Read [PLAYWRIGHT-PARITY.md](PLAYWRIGHT-PARITY.md) for findings, engine limitations, priorities and test evidence.','', 'This inventory covers every JavaScript-applicable method, property and event documented in the pinned upstream browser API, test API, reporter API, Electron API and Android API directories. Language-specific members are excluded; same-name overloads are collapsed within a member kind. Deprecated APIs and experimental APIs remain visible. Inherited members are represented on their declaring class. Arguments/options are reviewed by feature in the companion report, not counted as separate members.','', '| Status | Meaning |','|---|---|','| Equivalent | A counterpart exists for the basic operation/value; this is not a claim of full class/options/engine parity. |','| Partial | Related exposed operation, manual composition or field exists, with semantic/options/engine differences. |','| Idiomatic | Comparable checks/operations are expressed through Rust language/library facilities; no Playwright-style API object. |','| Missing | No dedicated counterpart found; arbitrary JS evaluation or raw CDP/BiDi calls do not establish feature parity. |','',f"Inventory: **{len(classes)} classes, {len(rows)} distinct members**. "+'; '.join(f'{k}: {counts[k]}' for k in ['Equivalent','Partial','Idiomatic','Missing'])+'. These counts are inventory labels, not a percentage of behavioral compatibility.','']
for c,entries in sorted(classes.items()):
 cr=[x for x in rows if x['cls']==c]
 md+=['## '+c,'']
 if not cr:md+=['No own JS-applicable member headings. The class/error type is not exposed as a Ferrite class; use `E2eError`.',''];continue
 file=entries[0]['source_file']
 url='https://github.com/microsoft/playwright/blob/v1.63.0/'+manifest[file]
 md += [f'[Pinned upstream definition]({url})','', '| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |','|---|---|---|---|---|']
 for x in cr:
  target='`'+x['target']+'`' if x['target'] else '—'
  if x['ref']:
   f,n=x['ref'];target+=f' ([source](../../{f}#L{n}))'
  # GitHub links for local file line anchors; absolute user links are used in final response.
  name='`'+x['name']+'`'+(' (deprecated)' if x['deprecated'] else '')
  md.append('| '+ ' | '.join([name,x['kind'],x['status'],target,x['note'].replace('|','\\|').replace('\n',' ')])+' |')
 md+=['']
DOCS.mkdir(parents=True,exist_ok=True)
(DOCS/'PLAYWRIGHT-API-MATRIX.md').write_text('\n'.join(md))
(P/'classified-inventory.json').write_text(json.dumps(rows,indent=2))
(P/'summary.json').write_text(json.dumps({'classes':len(classes),'members':len(rows),'counts':dict(counts)},indent=2))
print(json.dumps({'classes':len(classes),'members':len(rows),'counts':dict(counts)},indent=2))
