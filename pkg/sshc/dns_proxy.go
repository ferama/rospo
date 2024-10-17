package sshc

import (
	"encoding/binary"
	"fmt"
	"sync"

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
// to the underying ssh connection
func (p *DnsProxy) resolveDomain(msg *dns.Msg) ([]byte, error) {

	conn, err := p.sshConn.Client.Dial("tcp", p.remoteDnsServer)
	if err != nil {
		return nil, fmt.Errorf("unable to connect to remote dns server: %v", err)
	}

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

func (p *DnsProxy) handleDNSQuery(w dns.ResponseWriter, msg *dns.Msg) {
	// check if we have a valid question
	if len(msg.Question) == 0 {
		log.Printf("invalid DNS query: no question section")
		return
	}

	// Preserve the original transaction ID
	originalID := msg.Id

	// Resolve the domain through the proxy
	dnsResponse, err := p.resolveDomain(msg)
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

	w.WriteMsg(reply)
}

func (p *DnsProxy) run(net string) {
	server := &dns.Server{
		Addr:    p.proxyListenAddr,
		Net:     net,
		Handler: dns.DefaultServeMux,
	}

	err := server.ListenAndServe()
	defer server.Shutdown()
	if err != nil {
		log.Fatalf("failed to start server: %s\n ", err.Error())
	}
}

func (p *DnsProxy) Start() error {
	p.sshConn.ReadyWait()

	dns.HandleFunc(".", p.handleDNSQuery)

	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		p.run("udp")
		wg.Done()
	}()

	wg.Add(1)
	go func() {
		p.run("tcp")
		wg.Done()
	}()

	log.Printf("dns-proxy listening on: %s. Using remote dns: %s", p.proxyListenAddr, p.remoteDnsServer)
	wg.Wait()
	return nil
}
