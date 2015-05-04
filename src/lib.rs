#![feature(convert)]
#![feature(core)]
#![feature(collections)]
#![allow(trivial_casts)]

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

    AF_UNIX,
};


use std::iter::{FromIterator,};
use std::io::{Error, ErrorKind, Result,};
use std::mem;
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs, SocketAddrV4};
use std::num;
use std::num::Int;
use std::ops::Drop;
use std::vec::{Vec,};

use libc::{
    c_void, size_t, in_addr, sockaddr, sockaddr_in, socklen_t,
    c_int,

    socket, setsockopt, bind, send, recv, recvfrom,
    close,
    listen, sendto, accept, connect, getsockname,
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

extern {
    #[link_name="socketpair"]
    fn c_socketpair(domain: c_int, type_: c_int, protocol: c_int, sv: *mut [c_int]) -> c_int;
}


/// Converts a value from host byte order to network byte order.
#[inline]
pub fn htons(hostshort: u16) -> u16 {
    hostshort.to_be()
}


/// Converts a value from network byte order to host byte order.
#[inline]
pub fn ntohs(netshort: u16) -> u16 {
    Int::from_be(netshort)
}


/// Converts a value from host byte order to network byte order.
#[inline]
pub fn htonl(hostlong: u32) -> u32 {
    hostlong.to_be()
}


/// Converts a value from network byte order to host byte order.
#[inline]
pub fn ntohl(netlong: u32) -> u32 {
    Int::from_be(netlong)
}

pub fn socketpair(domain: i32, type_: i32, protocol: i32) -> Result<(Socket, Socket)> {
    unsafe {
        let mut fds: [c_int; 2] = mem::zeroed();
        _try!(c_socketpair, domain as c_int, type_ as c_int, protocol as c_int, &mut fds as *mut [c_int]);
        Ok((Socket { fd: fds[0] }, Socket { fd: fds[1] }))
    }
}


#[derive(Debug)]
pub struct Socket {
    fd: i32,
}

fn tosocketaddrs_to_socketaddr<T: ToSocketAddrs + ?Sized>(address: &T) -> Result<SocketAddr> {
    let addresses: Vec<SocketAddr> = FromIterator::from_iter(try!(address.to_socket_addrs()));

    match addresses.len() {
        1 => {
            Ok(addresses[0])
        },
        // TODO is this really possible?
        n => Err(Error::new(
            ErrorKind::InvalidInput,
            format!(
                "Incorrect number of IP addresses passed, \
                1 address expected, got {}", n).as_str(),
        ))
    }
}


fn tosocketaddrs_to_sockaddr<T: ToSocketAddrs + ?Sized>(address: &T) -> Result<sockaddr> {
    Ok(socketaddr_to_sockaddr(&try!(tosocketaddrs_to_socketaddr(address))))
}


impl Socket {
    pub fn new(socket_family: i32, socket_type: i32, protocol: i32) -> Result<Socket> {
        let fd = _try!(socket, socket_family, socket_type, protocol);
        Ok(Socket { fd: fd })
    }

    /// Returns the underlying file descriptor.
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

    /// Binds socket to an address
    pub fn bind<T: ToSocketAddrs + ?Sized>(&self, address: &T) -> Result<()> {
        let sa = try!(tosocketaddrs_to_sockaddr(address));
        _try!(bind, self.fd, &sa, num::cast(mem::size_of::<sockaddr>()).unwrap());
        Ok(())
    }

    pub fn getsockname(&self) -> Result<SocketAddr> {
        let mut sa: sockaddr = unsafe { mem::zeroed() };
        let mut len: socklen_t = mem::size_of::<sockaddr>() as socklen_t;
        _try!(getsockname, self.fd,
              &mut sa as *mut sockaddr, &mut len as *mut socklen_t);
        assert_eq!(len, mem::size_of::<sockaddr>() as socklen_t);

        Ok(sockaddr_to_socketaddr(&sa))
    }

    pub fn sendto<T: ToSocketAddrs + ?Sized>(&self, buffer: &[u8], flags: i32, address: &T)
            -> Result<usize> {
        let sa = try!(tosocketaddrs_to_sockaddr(address));
        let sent = _try!(
            sendto, self.fd, buffer.as_ptr() as *const c_void,
            buffer.len() as size_t, flags, &sa as *const sockaddr,
            num::cast(mem::size_of::<sockaddr>()).unwrap());
        Ok(sent as usize)
    }

    pub fn send(&self, buffer: &[u8], flags: i32)
            -> Result<usize> {
        let sent = _try!(
            send, self.fd, buffer.as_ptr() as *const c_void, buffer.len() as size_t, flags);
        Ok(sent as usize)
    }

    /// Receives data from a remote socket and returns it with the address of the socket.
    pub fn recvfrom(&self, bytes: usize, flags: i32) -> Result<(SocketAddr, Box<[u8]>)> {
        let mut a = Vec::with_capacity(bytes);

        // This is needed to get some actual elements in the vector, not just a capacity
        a.resize(bytes, 0u8);

        let (socket_addr, received) = try!(self.recvfrom_into(a.as_mut_slice(), flags));

        a.truncate(received);
        Ok((socket_addr, a.into_boxed_slice()))
    }

    /// Similar to `recvfrom` but receives to predefined buffer and returns the number
    /// of bytes read.
    pub fn recvfrom_into(&self, buffer: &mut [u8], flags: i32) -> Result<(SocketAddr, usize)> {
        let mut sa: sockaddr = unsafe { mem::zeroed() };
        let mut sa_len: socklen_t = mem::size_of::<sockaddr>() as socklen_t;
        let received = _try!(
            recvfrom, self.fd, buffer.as_ptr() as *mut c_void, buffer.len() as size_t, flags,
            &mut sa as *mut sockaddr, &mut sa_len as *mut socklen_t);
        assert_eq!(sa_len, mem::size_of::<sockaddr>() as socklen_t);
        Ok((sockaddr_to_socketaddr(&sa), received as usize))
    }

    /// Returns up to `bytes` bytes received from the remote socket.
    pub fn recv(&self, bytes: usize, flags: i32) -> Result<Box<[u8]>> {
        let mut a = Vec::with_capacity(bytes);

        // This is needed to get some actual elements in the vector, not just a capacity
        a.resize(bytes, 0u8);

        let received = try!(self.recv_into(a.as_mut_slice(), flags));

        a.truncate(received);
        Ok(a.into_boxed_slice())
    }

    /// Similar to `recv` but receives to predefined buffer and returns the number
    /// of bytes read.
    pub fn recv_into(&self, buffer: &mut [u8], flags: i32) -> Result<usize> {
        let received = _try!(recv, self.fd, buffer.as_ptr() as *mut c_void, buffer.len() as size_t, flags);
        Ok(received as usize)
    }

    pub fn connect<T: ToSocketAddrs + ?Sized>(&self, toaddress: &T) -> Result<()> {
        let address = try!(tosocketaddrs_to_sockaddr(toaddress));
        _try!(connect, self.fd, &address as *const sockaddr, num::cast(mem::size_of::<sockaddr>()).unwrap());
        Ok(())
    }

    pub fn listen(&self, backlog: i32) -> Result<()> {
        _try!(listen, self.fd, backlog);
        Ok(())
    }

    pub fn accept(&self) -> Result<(Socket, SocketAddr)> {
        let mut sa: sockaddr = unsafe { mem::zeroed() };
        let mut sa_len: socklen_t = mem::size_of::<sockaddr>() as socklen_t;

        let fd = _try!(
            accept, self.fd, &mut sa as *mut sockaddr, &mut sa_len as *mut socklen_t);
        assert_eq!(sa_len, mem::size_of::<sockaddr>() as socklen_t);
        Ok((Socket { fd: fd }, sockaddr_to_socketaddr(&sa)))
    }

    pub fn close(&self) -> Result<()> {
        _try!(close, self.fd);
        Ok(())
    }

    pub fn shutdown(&self, how: i32) -> Result<()> {
        _try!(shutdown, self.fd, how);
        Ok(())
    }
}


impl Drop for Socket {
    fn drop(&mut self) {
        let _ = self.close();
    }
}


fn socketaddr_to_sockaddr(addr: &SocketAddr) -> sockaddr {
    unsafe {
        match *addr {
            SocketAddr::V4(v4) => {
                let mut sa: sockaddr_in = mem::zeroed();
                sa.sin_family = num::cast(AF_INET).unwrap();
                sa.sin_port = htons(v4.port());
                sa.sin_addr = *(&v4.ip().octets() as *const u8 as *const in_addr);
                *(&sa as *const sockaddr_in as *const sockaddr)
            },
            SocketAddr::V6(_) => {
                panic!("Not supported");
                /*
                let mut sa: sockaddr_in6 = mem::zeroed();
                sa.sin6_family = AF_INET6 as u16;
                sa.sin6_port = htons(v6.port());
                (&sa as *const sockaddr_in6 as *const sockaddr)
                */
            },
        }
    }
}

fn sockaddr_to_socketaddr(sa: &sockaddr) -> SocketAddr {
    match sa.sa_family as i32 {
        AF_INET => {
            let sin: &sockaddr_in = unsafe { mem::transmute(sa) };
            let ip_parts: [u8; 4] = unsafe { mem::transmute(sin.sin_addr) };
            SocketAddr::V4(
                SocketAddrV4::new(Ipv4Addr::new(
                    ip_parts[0],
                    ip_parts[1],
                    ip_parts[2],
                    ip_parts[3],
                ),
                ntohs(sin.sin_port))
            )
        },
        AF_INET6 => {
            panic!("IPv6 not supported yet")
        },
        _ => {
            unreachable!("Should not happen")
        }
    }
}


#[cfg(test)]
mod tests {
    use std::thread;
    use super::{Socket, AF_UNIX, AF_INET, SOCK_STREAM, SOCK_DGRAM, SOL_SOCKET, SO_REUSEADDR, socketpair};
    use std::net::SocketAddr;
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

    #[test]
    fn getsockname_works() {
        let s = Socket::new(AF_INET, SOCK_DGRAM, 0).unwrap();
        s.bind("127.0.0.1:0").unwrap();
        if let SocketAddr::V4(v4) = s.getsockname().unwrap() {
            assert_eq!(v4.ip().octets(), [127, 0, 0, 1]);
        } else {
            panic!("getsockname() failed!");
        }
    }

    #[test]
    fn udp_communication_works() {
        let receiver = Socket::new(AF_INET, SOCK_DGRAM, 0).unwrap();
        receiver.bind("0.0.0.0:0").unwrap();
        let address = receiver.getsockname().unwrap();

        let sender = Socket::new(AF_INET, SOCK_DGRAM, 0).unwrap();

        assert_eq!(sender.sendto("abcd".as_bytes(), 0, &address).unwrap(), 4);
        let (_, received) = receiver.recvfrom(10, 0).unwrap();
        assert_eq!(received.len(), 4);
        // TODO: test the actual content
    }

    #[test]
    fn tcp_communication_works() {
        let listener = Socket::new(AF_INET, SOCK_STREAM, 0).unwrap();
        listener.bind("0.0.0.0:0").unwrap();
        listener.listen(10).unwrap();

        let address = listener.getsockname().unwrap();

        let guard = thread::scoped(move || {
            let (server, _) = listener.accept().unwrap();
            let data = server.recv(10, 0).unwrap();
            assert_eq!(data.len(), 4);
            // TODO: test the received content
        });

        let client = Socket::new(AF_INET, SOCK_STREAM, 0).unwrap();
        client.connect(&address).unwrap();
        let sent = client.send("abcd".as_bytes(), 0).unwrap();
        println!("c4");
        assert_eq!(sent, 4);
    }

    #[test]
    fn socketpair_and_unix_sockets_work() {
        let (s1, s2) = socketpair(AF_UNIX, SOCK_STREAM, 0).unwrap();
        let guard = thread::scoped(move || {
            let data = s1.recv(10, 0).unwrap();
            assert_eq!(data.len(), 4);
            // TODO: test the received content
        });

        let sent = s2.send("abcd".as_bytes(), 0).unwrap();
        assert_eq!(sent, 4);
    }
}
