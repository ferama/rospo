package sshc

import (
	"encoding/binary"
	"fmt"
	"net"

	"github.com/miekg/dns"
)

const DEFAULT_DNS_SERVER = "1.1.1.1:53"

type DnsProxy struct {
	sshConn         *SshConnection
	remoteDnsServer string
	proxyListenAddr string
}

func NewDnsProxy(sshConn *SshConnection, conf *DnsProxyConf) *DnsProxy {
	remoteDnsServer := DEFAULT_DNS_SERVER
	if conf.RemoteDnsAddress != nil {
		remoteDnsServer = *conf.RemoteDnsAddress
	}
	p := &DnsProxy{
		sshConn:         sshConn,
		proxyListenAddr: conf.ListenAddress,
		remoteDnsServer: remoteDnsServer,
	}

	return p
}

// resolveDomain sends a DNS query for a domain name over TCP
func (p *DnsProxy) resolveDomain(conn net.Conn, msg *dns.Msg) ([]byte, error) {
	// Pack the DNS message (with the original transaction ID)
	query, err := msg.Pack()
	if err != nil {
		return nil, fmt.Errorf("failed to pack DNS message: %v", err)
	}

	// dns over TCP requires the queryLength info to be written
	// before the query
	queryLength := make([]byte, 2)
	binary.BigEndian.PutUint16(queryLength, uint16(len(query)))

	if _, err := conn.Write(append(queryLength, query...)); err != nil {
		return nil, fmt.Errorf("failed to send DNS query: %v", err)
	}

	responseLengthBytes := make([]byte, 2)
	if _, err := conn.Read(responseLengthBytes); err != nil {
		return nil, fmt.Errorf("failed to read response length: %v", err)
	}
	responseLength := binary.BigEndian.Uint16(responseLengthBytes)

	response := make([]byte, responseLength)
	if _, err := conn.Read(response); err != nil {
		return nil, fmt.Errorf("failed to read DNS response: %v", err)
	}

	return response, nil
}

func (p *DnsProxy) handleDNSQuery(udpConn *net.UDPConn, clientAddr *net.UDPAddr, query []byte) {
	// Unpack the DNS message
	msg := new(dns.Msg)
	if err := msg.Unpack(query); err != nil {
		log.Printf("failed to unpack DNS query: %v", err)
		return
	}

	// Extract the domain name from the query
	if len(msg.Question) == 0 {
		log.Printf("invalid DNS query: no question section")
		return
	}

	originalID := msg.Id // Preserve the original transaction ID

	conn, err := p.sshConn.Client.Dial("tcp", p.remoteDnsServer)
	if err != nil {
		log.Printf("unable to connect to remote dns server: %v", err)
		return
	}
	// Resolve the domain through the proxy
	dnsResponse, err := p.resolveDomain(conn, msg)
	if err != nil {
		log.Printf("failed to resolve domain: %v", err)
		return
	}

	// Unpack the DNS response
	reply := new(dns.Msg)
	if err := reply.Unpack(dnsResponse); err != nil {
		log.Printf("failed to unpack DNS response: %v", err)
		return
	}

	// Set the original transaction ID back into the response
	reply.Id = originalID

	// Pack the modified response (with correct ID)
	finalResponse, err := reply.Pack()
	if err != nil {
		log.Printf("failed to pack final DNS response: %v", err)
		return
	}

	// Send the DNS response back to the client
	if _, err := udpConn.WriteToUDP(finalResponse, clientAddr); err != nil {
		log.Printf("failed to send DNS response: %v", err)
		return
	}
}

func (p *DnsProxy) Start() error {
	p.sshConn.ReadyWait()

	addr, err := net.ResolveUDPAddr("udp", p.proxyListenAddr)
	if err != nil {
		return fmt.Errorf("failed to resolve UDP address: %v", err)
	}

	udpConn, err := net.ListenUDP("udp", addr)
	if err != nil {
		return fmt.Errorf("failed to listen on UDP port 53: %v", err)
	}
	defer udpConn.Close()
	log.Printf("dns-proxy listening on UDP: %s. Using remote dns: %s", p.proxyListenAddr, p.remoteDnsServer)

	// Handle incoming DNS queries
	buf := make([]byte, 4096)
	for {
		n, clientAddr, err := udpConn.ReadFromUDP(buf)
		if err != nil {
			continue
		}
		go p.handleDNSQuery(udpConn, clientAddr, buf[:n])
	}
}
