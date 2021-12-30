use std::future::Future;

use ntex::connect::{self, Address, Connect, Connector};
use ntex::io::{utils::Boxed, IoBoxed};
use ntex::{service::Service, time::Seconds, util::ByteString, util::PoolId, util::PoolRef};

#[cfg(feature = "openssl")]
use ntex::connect::openssl::{self, SslConnector};

#[cfg(feature = "rustls")]
use ntex::connect::rustls::{self, ClientConfig};

use super::errors::ConnectError;
use super::{cmd, Client, SimpleClient};

/// Redis connector
pub struct RedisConnector<A, T> {
    address: A,
    connector: T,
    passwords: Vec<ByteString>,
    pool: PoolRef,
}

impl<A> RedisConnector<A, ()>
where
    A: Address + Clone,
{
    #[allow(clippy::new_ret_no_self)]
    /// Create new redis connector
    pub fn new(address: A) -> RedisConnector<A, Boxed<Connector<A>, Connect<A>>> {
        RedisConnector {
            address,
            passwords: Vec::new(),
            connector: Connector::default().seal(),
            pool: PoolId::P7.pool_ref(),
        }
    }
}

impl<A, T> RedisConnector<A, T> {
    /// Add redis auth password
    pub fn password<U>(mut self, password: U) -> Self
    where
        U: AsRef<str>,
    {
        self.passwords
            .push(ByteString::from(password.as_ref().to_string()));
        self
    }

    /// Set memory pool.
    ///
    /// Use specified memory pool for memory allocations. By default P7
    /// memory pool is used.
    pub fn memory_pool(mut self, id: PoolId) -> Self {
        self.pool = id.pool_ref();
        self
    }

    /// Use custom connector
    pub fn connector<Io, U>(self, connector: U) -> RedisConnector<A, Boxed<U, Connect<A>>>
    where
        U: Service<Connect<A>, Response = Io, Error = connect::ConnectError>,
        IoBoxed: From<Io>,
    {
        RedisConnector {
            connector: Boxed::new(connector),
            address: self.address,
            passwords: self.passwords,
            pool: self.pool,
        }
    }

    /// Use custom boxed connector
    pub fn boxed_connector<U>(self, connector: U) -> RedisConnector<A, U>
    where
        U: Service<Connect<A>, Response = IoBoxed, Error = connect::ConnectError>,
    {
        RedisConnector {
            connector,
            address: self.address,
            passwords: self.passwords,
            pool: self.pool,
        }
    }

    #[cfg(feature = "openssl")]
    /// Use openssl connector.
    pub fn openssl(
        self,
        connector: SslConnector,
    ) -> RedisConnector<A, Boxed<openssl::Connector<A>, Connect<A>>> {
        RedisConnector {
            address: self.address,
            passwords: self.passwords,
            connector: Boxed::new(openssl::Connector::new(connector)),
            pool: self.pool,
        }
    }

    #[cfg(feature = "rustls")]
    /// Use rustls connector.
    pub fn rustls(
        self,
        config: ClientConfig,
    ) -> RedisConnector<A, Boxed<rustls::Connector<A>, Connect<A>>> {
        RedisConnector {
            address: self.address,
            passwords: self.passwords,
            connector: Boxed::new(rustls::Connector::new(config)),
            pool: self.pool,
        }
    }
}

impl<A, T> RedisConnector<A, T>
where
    A: Address + Clone,
    T: Service<Connect<A>, Response = IoBoxed, Error = connect::ConnectError>,
{
    /// Connect to redis server and create shared client
    pub fn connect(&self) -> impl Future<Output = Result<Client, ConnectError>> {
        let pool = self.pool;
        let passwords = self.passwords.clone();
        let fut = self.connector.call(Connect::new(self.address.clone()));

        async move {
            let io = fut.await?;
            io.set_memory_pool(pool);
            io.set_disconnect_timeout(Seconds::ZERO.into());

            let client = Client::new(io);

            if passwords.is_empty() {
                Ok(client)
            } else {
                for password in passwords {
                    if client.exec(cmd::Auth(password)).await? {
                        return Ok(client);
                    }
                }
                Err(ConnectError::Unauthorized)
            }
        }
    }

    /// Connect to redis server and create simple client
    pub fn connect_simple(&self) -> impl Future<Output = Result<SimpleClient, ConnectError>> {
        let pool = self.pool;
        let passwords = self.passwords.clone();
        let fut = self.connector.call(Connect::new(self.address.clone()));

        async move {
            let io = fut.await?;
            io.set_memory_pool(pool);
            io.set_disconnect_timeout(Seconds::ZERO.into());

            let client = SimpleClient::new(io);

            if passwords.is_empty() {
                Ok(client)
            } else {
                for password in passwords {
                    if client.exec(cmd::Auth(password)).await? {
                        return Ok(client);
                    }
                }
                Err(ConnectError::Unauthorized)
            }
        }
    }
}
