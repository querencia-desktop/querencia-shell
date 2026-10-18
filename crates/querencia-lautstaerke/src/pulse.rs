//! The audio system through libpulse, the client library of the PulseAudio
//! protocol. PipeWire speaks it through `pipewire-pulse`, so the same code
//! serves PulseAudio and PipeWire.
//!
//! libpulse is opened at run time (`dlopen("libpulse.so.0")`), not linked:
//! the panel builds without a -dev package and starts without libpulse or
//! without an audio service; it then only shows „Kein Audiodienst“.
//!
//! libpulse runs its own event loop in a thread of its own (threaded main
//! loop). The server sends events for every change (subscription), and the
//! callbacks re-read only the affected list; nothing is polled. The
//! callbacks keep the [`State`] behind a mutex and wake the GTK main thread
//! through the `wake` function given to [`Pulse::start`]; the main thread
//! then reads [`Pulse::state`]. Requests from the main thread (volume, mute,
//! default device) take the main-loop lock, as libpulse requires.
//!
//! The struct layouts below are those of `pulse/introspect.h`,
//! `pulse/sample.h`, `pulse/channelmap.h` and `pulse/volume.h` of
//! PulseAudio 17 (Ubuntu 26.04: libpulse0 1:17.0); these layouts have been
//! stable since PulseAudio 1.0 (only fields at the end were added).

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::model::{CHANNELS_MAX, Connection, Device, State, Stream};

/// The library opened at run time.
pub const LIBRARY: &str = "libpulse.so.0";
/// Name of this client at the server (shown e.g. by `pactl list clients`).
const CLIENT_NAME: &CStr = c"Querencia Lautstärke";

#[repr(C)]
struct Mainloop {
    _private: [u8; 0],
}
#[repr(C)]
struct Context {
    _private: [u8; 0],
}
#[repr(C)]
struct Operation {
    _private: [u8; 0],
}
#[repr(C)]
struct Proplist {
    _private: [u8; 0],
}

#[repr(C)]
struct SampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}

#[repr(C)]
struct ChannelMap {
    channels: u8,
    map: [c_int; CHANNELS_MAX],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CVolume {
    channels: u8,
    values: [u32; CHANNELS_MAX],
}

impl CVolume {
    fn from_channels(channels: &[u32]) -> CVolume {
        let mut volume = CVolume {
            channels: channels.len().min(CHANNELS_MAX) as u8,
            values: [0; CHANNELS_MAX],
        };
        for (slot, value) in volume.values.iter_mut().zip(channels) {
            *slot = *value;
        }
        volume
    }

    fn channels(&self) -> Vec<u32> {
        self.values[..usize::from(self.channels).min(CHANNELS_MAX)].to_vec()
    }
}

/// `pa_sink_info`; `pa_source_info` has the same layout (there the
/// monitor fields are `monitor_of_sink` and `monitor_of_sink_name`).
#[repr(C)]
struct DeviceInfo {
    name: *const c_char,
    index: u32,
    description: *const c_char,
    sample_spec: SampleSpec,
    channel_map: ChannelMap,
    owner_module: u32,
    volume: CVolume,
    mute: c_int,
    monitor: u32,
    monitor_name: *const c_char,
    latency: u64,
    driver: *const c_char,
    flags: c_int,
    proplist: *mut Proplist,
    configured_latency: u64,
    base_volume: u32,
    state: c_int,
    n_volume_steps: u32,
    card: u32,
    n_ports: u32,
    ports: *mut c_void,
    active_port: *mut c_void,
    n_formats: u8,
    formats: *mut c_void,
}

#[repr(C)]
struct ServerInfo {
    user_name: *const c_char,
    host_name: *const c_char,
    server_version: *const c_char,
    server_name: *const c_char,
    sample_spec: SampleSpec,
    default_sink_name: *const c_char,
    default_source_name: *const c_char,
    cookie: u32,
    channel_map: ChannelMap,
}

#[repr(C)]
struct SinkInputInfo {
    index: u32,
    name: *const c_char,
    owner_module: u32,
    client: u32,
    sink: u32,
    sample_spec: SampleSpec,
    channel_map: ChannelMap,
    volume: CVolume,
    buffer_usec: u64,
    sink_usec: u64,
    resample_method: *const c_char,
    driver: *const c_char,
    mute: c_int,
    proplist: *mut Proplist,
    corked: c_int,
    has_volume: c_int,
    volume_writable: c_int,
    format: *mut c_void,
}

#[repr(C)]
struct SourceOutputInfo {
    index: u32,
    name: *const c_char,
    owner_module: u32,
    client: u32,
    source: u32,
    sample_spec: SampleSpec,
    channel_map: ChannelMap,
    buffer_usec: u64,
    source_usec: u64,
    resample_method: *const c_char,
    driver: *const c_char,
    proplist: *mut Proplist,
    corked: c_int,
    volume: CVolume,
    mute: c_int,
    has_volume: c_int,
    volume_writable: c_int,
    format: *mut c_void,
}

/// `PA_INVALID_INDEX`.
const INVALID_INDEX: u32 = u32::MAX;

// pa_context_state_t
const CONTEXT_READY: c_int = 4;
const CONTEXT_FAILED: c_int = 5;
const CONTEXT_TERMINATED: c_int = 6;
// pa_context_flags_t: never start a PulseAudio daemon of our own.
const CONTEXT_NOAUTOSPAWN: c_int = 0x0001;
// pa_subscription_mask_t: sinks, sources, sink inputs, source outputs,
// server (default devices).
const SUBSCRIPTION_MASK: c_int = 0x0001 | 0x0002 | 0x0004 | 0x0008 | 0x0080;
// pa_subscription_event_type_t
const FACILITY_MASK: c_int = 0x000F;

type StateCb = unsafe extern "C" fn(*mut Context, *mut c_void);
type SubscribeCb = unsafe extern "C" fn(*mut Context, c_int, u32, *mut c_void);
type SuccessCb = unsafe extern "C" fn(*mut Context, c_int, *mut c_void);
type ServerInfoCb = unsafe extern "C" fn(*mut Context, *const ServerInfo, *mut c_void);
type DeviceInfoCb = unsafe extern "C" fn(*mut Context, *const DeviceInfo, c_int, *mut c_void);
type SinkInputInfoCb = unsafe extern "C" fn(*mut Context, *const SinkInputInfo, c_int, *mut c_void);
type SourceOutputInfoCb =
    unsafe extern "C" fn(*mut Context, *const SourceOutputInfo, c_int, *mut c_void);

/// The libpulse functions used, looked up once.
struct Api {
    mainloop_new: unsafe extern "C" fn() -> *mut Mainloop,
    mainloop_get_api: unsafe extern "C" fn(*mut Mainloop) -> *mut c_void,
    mainloop_start: unsafe extern "C" fn(*mut Mainloop) -> c_int,
    mainloop_lock: unsafe extern "C" fn(*mut Mainloop),
    mainloop_unlock: unsafe extern "C" fn(*mut Mainloop),
    context_new: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut Context,
    context_set_state_callback: unsafe extern "C" fn(*mut Context, Option<StateCb>, *mut c_void),
    context_connect:
        unsafe extern "C" fn(*mut Context, *const c_char, c_int, *const c_void) -> c_int,
    context_disconnect: unsafe extern "C" fn(*mut Context),
    context_unref: unsafe extern "C" fn(*mut Context),
    context_get_state: unsafe extern "C" fn(*mut Context) -> c_int,
    context_errno: unsafe extern "C" fn(*mut Context) -> c_int,
    strerror: unsafe extern "C" fn(c_int) -> *const c_char,
    context_set_subscribe_callback:
        unsafe extern "C" fn(*mut Context, Option<SubscribeCb>, *mut c_void),
    context_subscribe:
        unsafe extern "C" fn(*mut Context, c_int, Option<SuccessCb>, *mut c_void) -> *mut Operation,
    get_server_info:
        unsafe extern "C" fn(*mut Context, Option<ServerInfoCb>, *mut c_void) -> *mut Operation,
    get_sink_info_list:
        unsafe extern "C" fn(*mut Context, Option<DeviceInfoCb>, *mut c_void) -> *mut Operation,
    get_source_info_list:
        unsafe extern "C" fn(*mut Context, Option<DeviceInfoCb>, *mut c_void) -> *mut Operation,
    get_sink_input_info_list:
        unsafe extern "C" fn(*mut Context, Option<SinkInputInfoCb>, *mut c_void) -> *mut Operation,
    get_source_output_info_list: unsafe extern "C" fn(
        *mut Context,
        Option<SourceOutputInfoCb>,
        *mut c_void,
    ) -> *mut Operation,
    set_sink_volume_by_name: unsafe extern "C" fn(
        *mut Context,
        *const c_char,
        *const CVolume,
        Option<SuccessCb>,
        *mut c_void,
    ) -> *mut Operation,
    set_source_volume_by_name: unsafe extern "C" fn(
        *mut Context,
        *const c_char,
        *const CVolume,
        Option<SuccessCb>,
        *mut c_void,
    ) -> *mut Operation,
    set_sink_mute_by_name: unsafe extern "C" fn(
        *mut Context,
        *const c_char,
        c_int,
        Option<SuccessCb>,
        *mut c_void,
    ) -> *mut Operation,
    set_source_mute_by_name: unsafe extern "C" fn(
        *mut Context,
        *const c_char,
        c_int,
        Option<SuccessCb>,
        *mut c_void,
    ) -> *mut Operation,
    set_default_sink: unsafe extern "C" fn(
        *mut Context,
        *const c_char,
        Option<SuccessCb>,
        *mut c_void,
    ) -> *mut Operation,
    set_default_source: unsafe extern "C" fn(
        *mut Context,
        *const c_char,
        Option<SuccessCb>,
        *mut c_void,
    ) -> *mut Operation,
    set_sink_input_volume: unsafe extern "C" fn(
        *mut Context,
        u32,
        *const CVolume,
        Option<SuccessCb>,
        *mut c_void,
    ) -> *mut Operation,
    set_source_output_volume: unsafe extern "C" fn(
        *mut Context,
        u32,
        *const CVolume,
        Option<SuccessCb>,
        *mut c_void,
    ) -> *mut Operation,
    set_sink_input_mute: unsafe extern "C" fn(
        *mut Context,
        u32,
        c_int,
        Option<SuccessCb>,
        *mut c_void,
    ) -> *mut Operation,
    set_source_output_mute: unsafe extern "C" fn(
        *mut Context,
        u32,
        c_int,
        Option<SuccessCb>,
        *mut c_void,
    ) -> *mut Operation,
    operation_unref: unsafe extern "C" fn(*mut Operation),
    proplist_gets: unsafe extern "C" fn(*mut Proplist, *const c_char) -> *const c_char,
}

impl Api {
    /// Opens libpulse and looks up every function; an error names what is
    /// missing.
    fn load() -> Result<&'static Api, String> {
        let library = CString::new(LIBRARY).expect("no NUL");
        // SAFETY: dlopen with a valid C string; the handle is never closed,
        // so the function pointers below stay valid for the whole process.
        let handle = unsafe { libc::dlopen(library.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        if handle.is_null() {
            // SAFETY: dlerror returns NULL or a C string valid until the next
            // dl* call in this thread.
            let error = unsafe { libc::dlerror() };
            let error = if error.is_null() {
                String::new()
            } else {
                // SAFETY: see above.
                unsafe { CStr::from_ptr(error) }
                    .to_string_lossy()
                    .into_owned()
            };
            return Err(format!("{LIBRARY} nicht ladbar: {error}"));
        }
        macro_rules! sym {
            ($name:literal) => {{
                // SAFETY: the handle is valid; the symbol name is a
                // NUL-terminated literal.
                let pointer = unsafe { libc::dlsym(handle, concat!($name, "\0").as_ptr().cast()) };
                if pointer.is_null() {
                    return Err(format!("{LIBRARY}: {} fehlt", $name));
                }
                // SAFETY: the C prototype of this libpulse function is the
                // field type it is assigned to (see the PulseAudio headers).
                unsafe { function(pointer) }
            }};
        }
        let api = Api {
            mainloop_new: sym!("pa_threaded_mainloop_new"),
            mainloop_get_api: sym!("pa_threaded_mainloop_get_api"),
            mainloop_start: sym!("pa_threaded_mainloop_start"),
            mainloop_lock: sym!("pa_threaded_mainloop_lock"),
            mainloop_unlock: sym!("pa_threaded_mainloop_unlock"),
            context_new: sym!("pa_context_new"),
            context_set_state_callback: sym!("pa_context_set_state_callback"),
            context_connect: sym!("pa_context_connect"),
            context_disconnect: sym!("pa_context_disconnect"),
            context_unref: sym!("pa_context_unref"),
            context_get_state: sym!("pa_context_get_state"),
            context_errno: sym!("pa_context_errno"),
            strerror: sym!("pa_strerror"),
            context_set_subscribe_callback: sym!("pa_context_set_subscribe_callback"),
            context_subscribe: sym!("pa_context_subscribe"),
            get_server_info: sym!("pa_context_get_server_info"),
            get_sink_info_list: sym!("pa_context_get_sink_info_list"),
            get_source_info_list: sym!("pa_context_get_source_info_list"),
            get_sink_input_info_list: sym!("pa_context_get_sink_input_info_list"),
            get_source_output_info_list: sym!("pa_context_get_source_output_info_list"),
            set_sink_volume_by_name: sym!("pa_context_set_sink_volume_by_name"),
            set_source_volume_by_name: sym!("pa_context_set_source_volume_by_name"),
            set_sink_mute_by_name: sym!("pa_context_set_sink_mute_by_name"),
            set_source_mute_by_name: sym!("pa_context_set_source_mute_by_name"),
            set_default_sink: sym!("pa_context_set_default_sink"),
            set_default_source: sym!("pa_context_set_default_source"),
            set_sink_input_volume: sym!("pa_context_set_sink_input_volume"),
            set_source_output_volume: sym!("pa_context_set_source_output_volume"),
            set_sink_input_mute: sym!("pa_context_set_sink_input_mute"),
            set_source_output_mute: sym!("pa_context_set_source_output_mute"),
            operation_unref: sym!("pa_operation_unref"),
            proplist_gets: sym!("pa_proplist_gets"),
        };
        Ok(Box::leak(Box::new(api)))
    }
}

/// A function pointer of type `F` from the address `dlsym` returned.
///
/// # Safety
/// `F` is an `extern "C" fn` type matching the C prototype of the symbol at
/// `pointer`, which is not NULL.
unsafe fn function<F: Copy>(pointer: *mut c_void) -> F {
    assert_eq!(std::mem::size_of::<F>(), std::mem::size_of::<*mut c_void>());
    // SAFETY: by the caller; sizes checked above.
    unsafe { std::mem::transmute_copy(&pointer) }
}

/// The five lists the state is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum List {
    Server = 0,
    Sinks = 1,
    Sources = 2,
    Playback = 3,
    Recording = 4,
}

impl List {
    const ALL: [List; 5] = [
        List::Server,
        List::Sinks,
        List::Sources,
        List::Playback,
        List::Recording,
    ];

    /// The list a subscription event of `facility` changes.
    fn of_facility(facility: c_int) -> Option<List> {
        match facility {
            0 => Some(List::Sinks),
            1 => Some(List::Sources),
            2 => Some(List::Playback),
            3 => Some(List::Recording),
            7 => Some(List::Server),
            _ => None,
        }
    }
}

/// What the callbacks share with the main thread.
struct Inner {
    state: State,
    /// The context the callbacks belong to (as an address); callbacks of an
    /// older context are ignored.
    context: usize,
    /// A list request is running.
    in_flight: [bool; 5],
    /// An event came while the request was running: read the list again.
    again: [bool; 5],
    /// Every list has been read once since the connection was made; until
    /// then the state stays `Connecting`.
    complete: [bool; 5],
    sinks: Vec<Device>,
    sources: Vec<Device>,
    playback: Vec<Stream>,
    recording: Vec<Stream>,
}

struct Shared {
    api: &'static Api,
    inner: Mutex<Inner>,
    /// A wake-up of the main thread is pending.
    woken: AtomicBool,
    wake: Box<dyn Fn() + Send + Sync>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn wake(&self) {
        if !self.woken.swap(true, Ordering::AcqRel) {
            (self.wake)();
        }
    }
}

/// A connection to the audio service, kept up for the whole process.
pub struct Pulse {
    shared: Arc<Shared>,
    mainloop: *mut Mainloop,
    /// Current context; only touched with the main-loop lock held.
    context: std::cell::Cell<*mut Context>,
}

/// Holds the libpulse main-loop lock while it lives.
struct Locked<'a>(&'a Pulse);

impl Drop for Locked<'_> {
    fn drop(&mut self) {
        // SAFETY: the main loop is valid for the life of `Pulse` and locked
        // by `Pulse::lock`.
        unsafe { (self.0.shared.api.mainloop_unlock)(self.0.mainloop) };
    }
}

impl Pulse {
    /// Loads libpulse, starts its event loop and connects. `wake` is called
    /// from libpulse's thread whenever the state changed; it must get the
    /// main thread to read [`Pulse::state`] (it is not called again until
    /// the main thread has read the state). Errors only if libpulse itself
    /// is missing; no audio service is a state ([`Connection::Unavailable`]).
    pub fn start(wake: impl Fn() + Send + Sync + 'static) -> Result<Pulse, String> {
        let api = Api::load()?;
        // SAFETY: plain constructor.
        let mainloop = unsafe { (api.mainloop_new)() };
        if mainloop.is_null() {
            return Err("pa_threaded_mainloop_new fehlgeschlagen".into());
        }
        let shared = Arc::new(Shared {
            api,
            inner: Mutex::new(Inner {
                state: State {
                    connection: Connection::Connecting,
                    ..State::default()
                },
                context: 0,
                in_flight: [false; 5],
                again: [false; 5],
                complete: [false; 5],
                sinks: Vec::new(),
                sources: Vec::new(),
                playback: Vec::new(),
                recording: Vec::new(),
            }),
            woken: AtomicBool::new(false),
            wake: Box::new(wake),
        });
        // SAFETY: valid main loop.
        if unsafe { (api.mainloop_start)(mainloop) } < 0 {
            return Err("pa_threaded_mainloop_start fehlgeschlagen".into());
        }
        let pulse = Pulse {
            shared,
            mainloop,
            context: std::cell::Cell::new(std::ptr::null_mut()),
        };
        pulse.connect();
        Ok(pulse)
    }

    fn lock(&self) -> Locked<'_> {
        // SAFETY: valid main loop; never called from libpulse's own thread
        // (only the main thread calls into `Pulse`).
        unsafe { (self.shared.api.mainloop_lock)(self.mainloop) };
        Locked(self)
    }

    /// The state as last reported. Also re-arms the wake-up.
    pub fn state(&self) -> State {
        self.shared.woken.store(false, Ordering::Release);
        self.shared.lock().state.clone()
    }

    /// Drops the current connection, if any, and connects anew (e.g. after
    /// the audio service came back).
    pub fn reconnect(&self) {
        self.connect();
    }

    fn connect(&self) {
        let api = self.shared.api;
        let _locked = self.lock();
        let old = self.context.replace(std::ptr::null_mut());
        if !old.is_null() {
            // SAFETY: `old` is our context; lock held. Callbacks are
            // removed first, so none runs for it any more.
            unsafe {
                (api.context_set_state_callback)(old, None, std::ptr::null_mut());
                (api.context_set_subscribe_callback)(old, None, std::ptr::null_mut());
                (api.context_disconnect)(old);
                (api.context_unref)(old);
            }
        }
        // SAFETY: valid main loop; the name is a C string.
        let context = unsafe {
            (api.context_new)((api.mainloop_get_api)(self.mainloop), CLIENT_NAME.as_ptr())
        };
        {
            let mut inner = self.shared.lock();
            inner.context = context as usize;
            inner.in_flight = [false; 5];
            inner.again = [false; 5];
            inner.complete = [false; 5];
            inner.state = State {
                connection: Connection::Connecting,
                ..State::default()
            };
        }
        if context.is_null() {
            self.shared.lock().state = State::unavailable("pa_context_new fehlgeschlagen");
            self.shared.wake();
            return;
        }
        self.context.set(context);
        let userdata = Arc::as_ptr(&self.shared) as *mut c_void;
        // SAFETY: lock held; `userdata` points to `Shared`, which lives as
        // long as `self` (the context is unreffed before it is dropped).
        unsafe {
            (api.context_set_state_callback)(context, Some(on_state), userdata);
            if (api.context_connect)(
                context,
                std::ptr::null(),
                CONTEXT_NOAUTOSPAWN,
                std::ptr::null(),
            ) < 0
            {
                let reason = error_text(api, context);
                self.shared.lock().state = State::unavailable(&reason);
                self.shared.wake();
            }
        }
    }

    /// Runs `request` on the context if it is connected, and frees the
    /// operation it returns. Returns whether it ran.
    fn request(&self, request: impl FnOnce(&Api, *mut Context) -> *mut Operation) -> bool {
        let api = self.shared.api;
        let _locked = self.lock();
        let context = self.context.get();
        // SAFETY: lock held, context valid while set.
        if context.is_null() || unsafe { (api.context_get_state)(context) } != CONTEXT_READY {
            return false;
        }
        let operation = request(api, context);
        if !operation.is_null() {
            // SAFETY: an operation returned by libpulse.
            unsafe { (api.operation_unref)(operation) };
        }
        true
    }

    /// Sets the volume of the output (`sink`) or input device `name`.
    pub fn set_device_volume(&self, sink: bool, name: &str, channels: &[u32]) -> bool {
        let Ok(name) = CString::new(name) else {
            return false;
        };
        let volume = CVolume::from_channels(channels);
        self.request(|api, context| unsafe {
            // SAFETY: lock held (see `request`); pointers valid for the call.
            let set = if sink {
                api.set_sink_volume_by_name
            } else {
                api.set_source_volume_by_name
            };
            set(context, name.as_ptr(), &volume, None, std::ptr::null_mut())
        })
    }

    /// Mutes or unmutes the output (`sink`) or input device `name`.
    pub fn set_device_muted(&self, sink: bool, name: &str, muted: bool) -> bool {
        let Ok(name) = CString::new(name) else {
            return false;
        };
        self.request(|api, context| unsafe {
            // SAFETY: as above.
            let set = if sink {
                api.set_sink_mute_by_name
            } else {
                api.set_source_mute_by_name
            };
            set(
                context,
                name.as_ptr(),
                c_int::from(muted),
                None,
                std::ptr::null_mut(),
            )
        })
    }

    /// Makes `name` the default output (`sink`) or input device.
    pub fn set_default(&self, sink: bool, name: &str) -> bool {
        let Ok(name) = CString::new(name) else {
            return false;
        };
        self.request(|api, context| unsafe {
            // SAFETY: as above.
            let set = if sink {
                api.set_default_sink
            } else {
                api.set_default_source
            };
            set(context, name.as_ptr(), None, std::ptr::null_mut())
        })
    }

    /// Sets the volume of a playing (`playback`) or recording stream.
    pub fn set_stream_volume(&self, playback: bool, index: u32, channels: &[u32]) -> bool {
        let volume = CVolume::from_channels(channels);
        self.request(|api, context| unsafe {
            // SAFETY: as above.
            let set = if playback {
                api.set_sink_input_volume
            } else {
                api.set_source_output_volume
            };
            set(context, index, &volume, None, std::ptr::null_mut())
        })
    }

    /// Mutes or unmutes a playing (`playback`) or recording stream.
    pub fn set_stream_muted(&self, playback: bool, index: u32, muted: bool) -> bool {
        self.request(|api, context| unsafe {
            // SAFETY: as above.
            let set = if playback {
                api.set_sink_input_mute
            } else {
                api.set_source_output_mute
            };
            set(
                context,
                index,
                c_int::from(muted),
                None,
                std::ptr::null_mut(),
            )
        })
    }
}

/// The text of the context's last error.
///
/// # Safety
/// `context` is valid and the main-loop lock is held.
unsafe fn error_text(api: &Api, context: *mut Context) -> String {
    // SAFETY: by the caller; pa_strerror returns a static string.
    unsafe { text((api.strerror)((api.context_errno)(context))) }
}

/// An owned copy of a C string; empty for NULL.
///
/// # Safety
/// `pointer` is NULL or a valid C string.
unsafe fn text(pointer: *const c_char) -> String {
    if pointer.is_null() {
        String::new()
    } else {
        // SAFETY: by the caller.
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    }
}

/// The `Shared` behind a callback's userdata, if the callback belongs to the
/// current context.
///
/// # Safety
/// `userdata` is the pointer `Pulse::connect` registered.
unsafe fn shared<'a>(userdata: *mut c_void, context: *mut Context) -> Option<&'a Shared> {
    // SAFETY: by the caller; `Shared` outlives every registered context.
    let shared = unsafe { &*(userdata as *const Shared) };
    (shared.lock().context == context as usize).then_some(shared)
}

/// Starts reading `list` again, or marks it to be read again once the
/// running request is done.
///
/// # Safety
/// Called from libpulse's thread (lock held) with a valid context.
unsafe fn refresh(shared: &Shared, context: *mut Context, list: List) {
    {
        let mut inner = shared.lock();
        if inner.in_flight[list as usize] {
            inner.again[list as usize] = true;
            return;
        }
        inner.in_flight[list as usize] = true;
        inner.again[list as usize] = false;
        match list {
            List::Sinks => inner.sinks.clear(),
            List::Sources => inner.sources.clear(),
            List::Playback => inner.playback.clear(),
            List::Recording => inner.recording.clear(),
            List::Server => {}
        }
    }
    let api = shared.api;
    let userdata = shared as *const Shared as *mut c_void;
    // SAFETY: by the caller.
    let operation = unsafe {
        match list {
            List::Server => (api.get_server_info)(context, Some(on_server), userdata),
            List::Sinks => (api.get_sink_info_list)(context, Some(on_sink), userdata),
            List::Sources => (api.get_source_info_list)(context, Some(on_source), userdata),
            List::Playback => (api.get_sink_input_info_list)(context, Some(on_playback), userdata),
            List::Recording => {
                (api.get_source_output_info_list)(context, Some(on_recording), userdata)
            }
        }
    };
    if operation.is_null() {
        shared.lock().in_flight[list as usize] = false;
    } else {
        // SAFETY: an operation returned by libpulse.
        unsafe { (api.operation_unref)(operation) };
    }
}

/// Ends reading `list`: puts what was read into the state, reads again if
/// an event came meanwhile, and wakes the main thread.
///
/// # Safety
/// As [`refresh`].
unsafe fn finished(shared: &Shared, context: *mut Context, list: List) {
    let again = {
        let mut inner = shared.lock();
        let inner = &mut *inner;
        inner.in_flight[list as usize] = false;
        inner.complete[list as usize] = true;
        match list {
            List::Sinks => inner.state.sinks = std::mem::take(&mut inner.sinks),
            List::Sources => inner.state.sources = std::mem::take(&mut inner.sources),
            List::Playback => inner.state.playback = std::mem::take(&mut inner.playback),
            List::Recording => inner.state.recording = std::mem::take(&mut inner.recording),
            List::Server => {}
        }
        if inner.complete.iter().all(|c| *c) {
            inner.state.connection = Connection::Ready;
        }
        inner.again[list as usize]
    };
    if again {
        // SAFETY: by the caller.
        unsafe { refresh(shared, context, list) };
    }
    shared.wake();
}

unsafe extern "C" fn on_state(context: *mut Context, userdata: *mut c_void) {
    // SAFETY: registered by `Pulse::connect`; runs in libpulse's thread.
    let Some(shared) = (unsafe { shared(userdata, context) }) else {
        return;
    };
    let api = shared.api;
    // SAFETY: valid context in its own callback.
    match unsafe { (api.context_get_state)(context) } {
        CONTEXT_READY => {
            // SAFETY: as above.
            unsafe {
                (api.context_set_subscribe_callback)(context, Some(on_event), userdata);
                let operation =
                    (api.context_subscribe)(context, SUBSCRIPTION_MASK, None, std::ptr::null_mut());
                if !operation.is_null() {
                    (api.operation_unref)(operation);
                }
                for list in List::ALL {
                    refresh(shared, context, list);
                }
            }
        }
        CONTEXT_FAILED | CONTEXT_TERMINATED => {
            // SAFETY: as above.
            let reason = unsafe { error_text(api, context) };
            shared.lock().state = State::unavailable(&reason);
            shared.wake();
        }
        _ => {}
    }
}

unsafe extern "C" fn on_event(
    context: *mut Context,
    event: c_int,
    _index: u32,
    userdata: *mut c_void,
) {
    // SAFETY: as in `on_state`.
    let Some(shared) = (unsafe { shared(userdata, context) }) else {
        return;
    };
    if let Some(list) = List::of_facility(event & FACILITY_MASK) {
        // SAFETY: as above.
        unsafe { refresh(shared, context, list) };
    }
}

unsafe extern "C" fn on_server(
    context: *mut Context,
    info: *const ServerInfo,
    userdata: *mut c_void,
) {
    // SAFETY: as in `on_state`.
    let Some(shared) = (unsafe { shared(userdata, context) }) else {
        return;
    };
    if !info.is_null() {
        // SAFETY: libpulse passes a valid info for the call.
        let info = unsafe { &*info };
        let mut inner = shared.lock();
        // SAFETY: C strings of a valid info.
        unsafe {
            let name = text(info.server_name);
            let version = text(info.server_version);
            inner.state.server = format!("{name} {version}").trim().to_string();
            inner.state.default_sink = Some(text(info.default_sink_name)).filter(|s| !s.is_empty());
            inner.state.default_source =
                Some(text(info.default_source_name)).filter(|s| !s.is_empty());
        }
    }
    // SAFETY: as above.
    unsafe { finished(shared, context, List::Server) };
}

/// A `Device` of a sink or source info.
///
/// # Safety
/// `info` is valid for the call.
unsafe fn device(info: &DeviceInfo, sink: bool) -> Device {
    // SAFETY: by the caller.
    unsafe {
        Device {
            index: info.index,
            name: text(info.name),
            description: text(info.description),
            volume: info.volume.channels(),
            muted: info.mute != 0,
            // For a source: the sink it monitors; a sink's field is its own
            // monitor source.
            monitor: !sink && info.monitor != INVALID_INDEX,
        }
    }
}

unsafe extern "C" fn on_sink(
    context: *mut Context,
    info: *const DeviceInfo,
    eol: c_int,
    userdata: *mut c_void,
) {
    // SAFETY: as in `on_state`.
    unsafe { on_device(context, info, eol, userdata, true) }
}

unsafe extern "C" fn on_source(
    context: *mut Context,
    info: *const DeviceInfo,
    eol: c_int,
    userdata: *mut c_void,
) {
    // SAFETY: as in `on_state`.
    unsafe { on_device(context, info, eol, userdata, false) }
}

/// # Safety
/// As the list callbacks.
unsafe fn on_device(
    context: *mut Context,
    info: *const DeviceInfo,
    eol: c_int,
    userdata: *mut c_void,
    sink: bool,
) {
    // SAFETY: as in `on_state`.
    let Some(shared) = (unsafe { shared(userdata, context) }) else {
        return;
    };
    let list = if sink { List::Sinks } else { List::Sources };
    if eol != 0 || info.is_null() {
        // SAFETY: as above.
        unsafe { finished(shared, context, list) };
        return;
    }
    // SAFETY: valid for the call.
    let device = unsafe { device(&*info, sink) };
    let mut inner = shared.lock();
    if sink {
        inner.sinks.push(device);
    } else {
        inner.sources.push(device);
    }
}

/// The fields a sink input and a source output share.
struct RawStream<'a> {
    index: u32,
    name: *const c_char,
    proplist: *mut Proplist,
    /// The sink or source.
    device: u32,
    volume: &'a CVolume,
    mute: c_int,
    has_volume: c_int,
    volume_writable: c_int,
}

/// A `Stream` from the common fields of a sink input or source output.
///
/// # Safety
/// The pointers in `raw` are valid for the call.
unsafe fn stream(api: &Api, raw: RawStream<'_>) -> Stream {
    let property = |key: &CStr| {
        if raw.proplist.is_null() {
            return String::new();
        }
        // SAFETY: valid proplist for the call.
        unsafe { text((api.proplist_gets)(raw.proplist, key.as_ptr())) }
    };
    // SAFETY: by the caller.
    let name = unsafe { text(raw.name) };
    let application = Some(property(c"application.name"))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| name.clone());
    let title = Some(property(c"media.name"))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(name);
    Stream {
        index: raw.index,
        application,
        title,
        icon: Some(property(c"application.icon_name")).filter(|s| !s.is_empty()),
        device: raw.device,
        volume: raw.volume.channels(),
        muted: raw.mute != 0,
        adjustable: raw.has_volume != 0 && raw.volume_writable != 0,
    }
}

unsafe extern "C" fn on_playback(
    context: *mut Context,
    info: *const SinkInputInfo,
    eol: c_int,
    userdata: *mut c_void,
) {
    // SAFETY: as in `on_state`.
    let Some(shared) = (unsafe { shared(userdata, context) }) else {
        return;
    };
    if eol != 0 || info.is_null() {
        // SAFETY: as above.
        unsafe { finished(shared, context, List::Playback) };
        return;
    }
    // SAFETY: valid for the call.
    let stream = unsafe {
        let i = &*info;
        stream(
            shared.api,
            RawStream {
                index: i.index,
                name: i.name,
                proplist: i.proplist,
                device: i.sink,
                volume: &i.volume,
                mute: i.mute,
                has_volume: i.has_volume,
                volume_writable: i.volume_writable,
            },
        )
    };
    shared.lock().playback.push(stream);
}

unsafe extern "C" fn on_recording(
    context: *mut Context,
    info: *const SourceOutputInfo,
    eol: c_int,
    userdata: *mut c_void,
) {
    // SAFETY: as in `on_state`.
    let Some(shared) = (unsafe { shared(userdata, context) }) else {
        return;
    };
    if eol != 0 || info.is_null() {
        // SAFETY: as above.
        unsafe { finished(shared, context, List::Recording) };
        return;
    }
    // SAFETY: valid for the call.
    let stream = unsafe {
        let i = &*info;
        stream(
            shared.api,
            RawStream {
                index: i.index,
                name: i.name,
                proplist: i.proplist,
                device: i.source,
                volume: &i.volume,
                mute: i.mute,
                has_volume: i.has_volume,
                volume_writable: i.volume_writable,
            },
        )
    };
    shared.lock().recording.push(stream);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    /// Offsets on x86_64 (LP64) as libpulse 17 lays the structs out; the
    /// end-to-end check is the isolated test with a real server, these
    /// guard against accidental edits.
    #[test]
    fn struct_layouts_match_libpulse_on_lp64() {
        assert_eq!(size_of::<CVolume>(), 132);
        assert_eq!(size_of::<ChannelMap>(), 132);
        assert_eq!(size_of::<SampleSpec>(), 12);
        assert_eq!(offset_of!(DeviceInfo, sample_spec), 24);
        assert_eq!(offset_of!(DeviceInfo, channel_map), 36);
        assert_eq!(offset_of!(DeviceInfo, owner_module), 168);
        assert_eq!(offset_of!(DeviceInfo, volume), 172);
        assert_eq!(offset_of!(DeviceInfo, mute), 304);
        assert_eq!(offset_of!(DeviceInfo, monitor), 308);
        assert_eq!(offset_of!(DeviceInfo, monitor_name), 312);
        assert_eq!(offset_of!(ServerInfo, default_sink_name), 48);
        assert_eq!(offset_of!(SinkInputInfo, volume), 172);
        assert_eq!(offset_of!(SinkInputInfo, mute), 336);
        assert_eq!(offset_of!(SourceOutputInfo, volume), 220);
        assert_eq!(offset_of!(SourceOutputInfo, mute), 352);
    }

    #[test]
    fn cvolume_keeps_at_most_32_channels() {
        let v = CVolume::from_channels(&[7; 40]);
        assert_eq!(v.channels, 32);
        assert_eq!(v.channels(), vec![7; 32]);
        assert_eq!(CVolume::from_channels(&[1, 2]).channels(), vec![1, 2]);
    }

    #[test]
    fn events_map_to_the_lists_they_change() {
        assert_eq!(List::of_facility(0), Some(List::Sinks));
        assert_eq!(List::of_facility(0x10 & FACILITY_MASK), Some(List::Sinks));
        assert_eq!(List::of_facility(0x21 & FACILITY_MASK), Some(List::Sources));
        assert_eq!(List::of_facility(2), Some(List::Playback));
        assert_eq!(List::of_facility(3), Some(List::Recording));
        assert_eq!(List::of_facility(7), Some(List::Server));
        assert_eq!(List::of_facility(5), None); // clients
        assert_eq!(List::of_facility(9), None); // cards
    }
}
