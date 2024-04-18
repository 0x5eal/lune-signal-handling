use std::{process::ExitCode, rc::Weak};

use mlua::prelude::*;
use mlua_luau_scheduler::{LuaSchedulerExt, LuaSpawnExt};
use tokio::signal::unix::{self, signal};

type DefaultSignalHandler = fn(&Lua, ()) -> LuaResult<()>;

pub async fn signal_listen(
    lua: &Lua,
    (sig_kind, handler_key): (SignalKind<DefaultSignalHandler>, LuaRegistryKey),
) -> LuaResult<()> {
    let mut sig = signal(sig_kind.into())?;

    let lua_inner = lua
        .app_data_ref::<Weak<Lua>>()
        .expect("Missing weak lua ref")
        .upgrade()
        .expect("Lua was dropped unexpectedly");

    // NOTE: This a signal handler will never be fired when the main thread is executing
    // since we're using `spawn_local`. We should warn the users that hot loops would prevent
    // signal handling
    lua.spawn_local(async move {
        loop {
            sig.recv().await;

            let handler_fn = lua_inner
                .registry_value::<LuaFunction<'_>>(&handler_key)
                .unwrap();

            handler_fn
                .call_async::<_, ()>(lua_inner.create_function(sig_kind.get_inner()))
                .await
                .unwrap();
        }
    });

    Ok(())
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy)]
pub enum SignalKind<T: Fn(&Lua, ()) -> LuaResult<()> + Send + Sync + 'static> {
    SIGALRM(T),
    SIGCHLD(T),
    SIGHUP(T),
    SIGINT(T),
    SIGIO(T),
    SIGPIPE(T),
    SIGQUIT(T),
    SIGTERM(T),
    SIGUSR1(T),
    SIGUSR2(T),
    SIGWINCH(T),
}

impl SignalKind<DefaultSignalHandler> {
    fn get_inner(self) -> DefaultSignalHandler {
        match self {
            SignalKind::SIGALRM(x) => x,
            SignalKind::SIGCHLD(x) => x,
            SignalKind::SIGHUP(x) => x,
            SignalKind::SIGINT(x) => x,
            SignalKind::SIGIO(x) => x,
            SignalKind::SIGPIPE(x) => x,
            SignalKind::SIGQUIT(x) => x,
            SignalKind::SIGTERM(x) => x,
            SignalKind::SIGUSR1(x) => x,
            SignalKind::SIGUSR2(x) => x,
            SignalKind::SIGWINCH(x) => x,
        }
    }
}
impl From<SignalKind<DefaultSignalHandler>> for unix::SignalKind {
    fn from(val: SignalKind<DefaultSignalHandler>) -> Self {
        match val {
            SignalKind::SIGALRM(_) => unix::SignalKind::alarm(),
            SignalKind::SIGCHLD(_) => unix::SignalKind::child(),
            SignalKind::SIGHUP(_) => unix::SignalKind::hangup(),
            SignalKind::SIGINT(_) => unix::SignalKind::interrupt(),
            SignalKind::SIGIO(_) => unix::SignalKind::io(),
            SignalKind::SIGPIPE(_) => unix::SignalKind::pipe(),
            SignalKind::SIGQUIT(_) => unix::SignalKind::quit(),
            SignalKind::SIGTERM(_) => unix::SignalKind::terminate(),
            SignalKind::SIGUSR1(_) => unix::SignalKind::user_defined1(),
            SignalKind::SIGUSR2(_) => unix::SignalKind::user_defined2(),
            SignalKind::SIGWINCH(_) => unix::SignalKind::window_change(),
        }
    }
}

macro_rules! impl_default_term_handler {
    ($name:ident) => {
        fn $name(lua: &Lua, _: ()) -> LuaResult<()> {
            lua.set_exit_code(ExitCode::from(
                unix::SignalKind::$name().as_raw_value() as u8
            ));

            Ok(())
        }
    };
}

impl_default_term_handler!(alarm);
impl_default_term_handler!(hangup);
impl_default_term_handler!(interrupt);
impl_default_term_handler!(pipe);
impl_default_term_handler!(quit);
impl_default_term_handler!(terminate);

fn do_nothing(_: &Lua, _: ()) -> LuaResult<()> {
    Ok(())
}

impl<'lua> FromLua<'lua> for SignalKind<DefaultSignalHandler> {
    fn from_lua(value: LuaValue<'lua>, _: &'lua Lua) -> LuaResult<Self> {
        if let LuaValue::String(str) = value {
            return match str.to_str()? {
                "SIGALRM" => Ok(Self::SIGALRM(alarm)),
                "SIGCHLD" => Ok(Self::SIGCHLD(do_nothing)),
                "SIGHUP" => Ok(Self::SIGHUP(hangup)),
                "SIGINT" => Ok(Self::SIGINT(interrupt)),
                "SIGIO" => Ok(Self::SIGIO(do_nothing)),
                "SIGPIPE" => Ok(Self::SIGPIPE(pipe)),
                "SIGQUIT" => Ok(Self::SIGQUIT(quit)),
                "SIGTERM" => Ok(Self::SIGTERM(terminate)),
                "SIGUSR1" => Ok(Self::SIGUSR1(do_nothing)),
                "SIGUSR2" => Ok(Self::SIGUSR2(do_nothing)),
                "SIGWINCH" => Ok(Self::SIGWINCH(do_nothing)),
                &_ => Err(LuaError::runtime("Expected valid SignalKind")),
            };
        }

        println!("{:?}", value);

        Err(LuaError::runtime("SignalKind must be of type string"))
    }
}
