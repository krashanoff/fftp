# Fast File Transfer Protocol

FFTP is a toy-protocol framework over UDP -- like QUIC, but a little easier to
understand. It makes no guarantees about in-order delivery, but does make
guarantees of data integrity and security between connected parties.

## Handshake + Security

FFTP uses a 1RTT handshake process. The initiating host sends a special
frame called the [Initiate] frame. This frame contains the initiating host's
chosen public key for the connection.

The responding host uses this frame to compute their shared session key, then
replies with another frame, the [First] frame.

The [First] frame is encrypted to the initiating host with their public
key, containing whatever data the responding host wishes to send, along with their
public key.

The initiating host and responding host now both have copies of the shared private
key, allowing them to form their own copy of the session key.

## Data Integrity

Data integrity is achieved by a SHA256 checksum computed in each frame.

## Serialization

Messages are serialized on-the-wire in accordance with the [bincode format specification](https://github.com/bincode-org/bincode/blob/trunk/docs/spec.md).

They are sent in big endian byte order, with an imposed limit of the UDP frame size:
65536 bytes.

## Further Considerations

The Fast File Transfer Protocol is designed to be easy-to-understand, and guarantees
one level of encrypted communications between parties. Authentication is not provided,
and should be handled at the application level. On wired local area networks, the
identity of connected parties is assumed validated by hostname and local IP.
