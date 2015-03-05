#![feature(core)]
#![feature(io)]
#![feature(net)]

extern crate libc;

pub use libc::{
    AF_INET, AF_INET6, SOCK_STREAM, SOCK_DGRAM, SOCK_RAW,
    IPPROTO_IP, IPPROTO_IPV6, IPPROTO_TCP, TCP_NODELAY,
    SOL_SOCKET, SO_KEEPALIVE, SO_ERROR,
    SO_REUSEADDR, SO_BROADCAST, SHUT_WR, IP_MULTICAST_LOOP,
    IP_ADD_MEMBERSHIP, IP_DROP_MEMBERSHIP,
    IPV6_ADD_MEMBERSHIP, IPV6_DROP_MEMBERSHIP,
    IP_MULTICAST_TTL, IP_TTL, IP_HDRINCL, SHUT_RD,
    IPPROTO_RAW,
};


use std::result::Result as GenericResult;
use std::iter::{FromIterator,};
use std::io::{Error, ErrorKind, Result,};
use std::os::{errno, error_string,};
use std::mem;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs,};
use std::num::Int;
use std::vec::{Vec,};

use libc::{
    c_void, in_addr, sockaddr, sockaddr_in, sockaddr_in6, socklen_t,

    socket, setsockopt, bind, send, recv, recvfrom,
    listen, sendto, accept, connect, getpeername, getsockname,
    shutdown,
};

macro_rules! _try {
    ( $fun:ident, $( $x:expr ),* ) => {{
        let value = unsafe { $fun($($x,)*) };
        if value == -1 {
            return Err(Error::last_os_error());
        }
        value
    }};
}


/// Convert a value from host byte order to network byte order
pub fn htons(hostshort: u16) -> u16 {
    hostshort.to_be()
}


/// Convert a value from network byte order to host byte order
pub fn ntohs(netshort: u16) -> u16 {
    Int::from_be(netshort)
}


/// Convert a value from host byte order to network byte order
pub fn htonl(hostlong: u32) -> u32 {
    hostlong.to_be()
}


/// Convert a value from network byte order to host byte order
pub fn ntohl(netlong: u32) -> u32 {
    Int::from_be(netlong)
}


#[derive(Debug)]
pub struct Socket {
    fd: i32,
}

impl Socket {
    pub fn new(socket_family: i32, socket_type: i32, protocol: i32) -> Result<Socket> {
        let fd = _try!(socket, socket_family, socket_type, protocol);
        Ok(Socket { fd: fd })
    }

    pub fn fileno(&self) -> i32 {
        self.fd
    }

    pub fn setsockopt<T>(&self, level: i32, name: i32, value: T) -> Result<()> {
        unsafe {
            let value = &value as *const T as *const c_void;
            _try!(
                setsockopt,
                self.fd, level, name, value, mem::size_of::<T>() as socklen_t);
        }
        Ok(())
    }

    pub fn bind<T: ToSocketAddrs + ?Sized>(&self, address: &T) -> Result<()> {
        let addresses: Vec<SocketAddr> =
            FromIterator::from_iter(try!(address.to_socket_addrs()));

        match addresses.len() {
            1 => {
                let a = addresses[0];
                let sa = socketaddr_to_sockaddr(&a);
                _try!(bind, self.fd, &sa, mem::size_of::<sockaddr>() as i32);
                Ok(())
            },
            // TODO is this really possible? If yes - should be refactored out
            // of here
            n => Err(Error::new(
                ErrorKind::InvalidInput,
                "Incorrect number of IP addresses passed",
                Some(format!("1 address expected, got {}", n)),
            ))
        }
    }
}

fn socketaddr_to_sockaddr(addr: &SocketAddr) -> sockaddr {
    unsafe {
        match addr.ip() {
            IpAddr::V4(v4) => {
                let mut sa: sockaddr_in = mem::zeroed();
                sa.sin_family = AF_INET as u8;
                sa.sin_port = htons(addr.port());
                sa.sin_addr = *(&v4.octets() as *const u8 as *const in_addr);
                *(&sa as *const sockaddr_in as *const sockaddr)
            },
            IpAddr::V6(_) => {
                let mut sa: sockaddr_in6 = mem::zeroed();
                sa.sin6_family = AF_INET6 as u8;
                sa.sin6_port= htons(addr.port());
                panic!("Not supported");
                *(&sa as *const sockaddr_in6 as *const sockaddr)
            },
        }
    }
}


/*
#[test]
fn inet_aton_works() {
    assert_eq!(inet_aton("1.2.3.4"), Ok([1u8, 2u8, 3u8, 4u8]));
}
*/

#[test]
fn some_basic_socket_stuff_works() {
    let socket = Socket::new(AF_INET, SOCK_DGRAM, 0).unwrap();
    socket.setsockopt(SOL_SOCKET, SO_REUSEADDR, 1).unwrap();
    socket.bind("0.0.0.0:0").unwrap();
}
