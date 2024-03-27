package utils

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"encoding/base64"
	"encoding/pem"
	"fmt"
	"log"
	"net"
	"os"
	"path/filepath"

	"golang.org/x/crypto/ssh"
)

// GeneratePrivateKey generate an rsa key (actually used from the sshd server)
func GeneratePrivateKey() (*ecdsa.PrivateKey, error) {
	privateKey, _ := ecdsa.GenerateKey(elliptic.P521(), rand.Reader)

	// log.Println("private key generated")
	return privateKey, nil
}

// EncodePrivateKeyToPEM converts a private key object to PEM
func EncodePrivateKeyToPEM(privateKey *ecdsa.PrivateKey) []byte {
	encoded, _ := x509.MarshalECPrivateKey(privateKey)

	// pem.Block
	privBlock := pem.Block{
		Type:    "EC PRIVATE KEY",
		Headers: nil,
		Bytes:   encoded,
	}

	// Private key in PEM format
	privatePEM := pem.EncodeToMemory(&privBlock)
	return privatePEM
}

// GeneratePublicKey generates a public key from a private one
func GeneratePublicKey(key *ecdsa.PublicKey) ([]byte, error) {
	publicRsaKey, err := ssh.NewPublicKey(key)
	if err != nil {
		return nil, err
	}

	pubKeyBytes := ssh.MarshalAuthorizedKey(publicRsaKey)

	return pubKeyBytes, nil
}

// WriteKeyToFile stores a key to the specified path
func WriteKeyToFile(keyBytes []byte, keyPath string) error {
	path, _ := ExpandUserHome(keyPath)

	if err := os.WriteFile(path, keyBytes, 0600); err != nil {
		log.Println(err)
		return err
	}
	return nil
}

// LoadIdentityFile reads a public key file and loads the keys to
// an ssh.PublicKeys object
func LoadIdentityFile(file string) (ssh.AuthMethod, error) {
	path, _ := ExpandUserHome(file)

	usr := CurrentUser()
	// no path is set, try with a reasonable default
	if path == "" {
		path = filepath.Join(usr.HomeDir, ".ssh", "id_rsa")
	}

	buffer, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("cannot read SSH idendity key file %s", path)
	}

	key, err := ssh.ParsePrivateKey(buffer)
	if err != nil {
		return nil, fmt.Errorf("cannot parse SSH identity key file %s", file)
	}

	return ssh.PublicKeys(key), nil
}

// AddHostKeyToKnownHosts updates user known_hosts file adding the host key
func AddHostKeyToKnownHosts(host string, key ssh.PublicKey, knownHostsPath string) error {
	// add host key if host is not found in known_hosts, error object is return, if nil then connection proceeds,
	// if not nil then connection stops.

	f, fErr := os.OpenFile(knownHostsPath, os.O_APPEND|os.O_WRONLY, 0600)
	if fErr != nil {
		return fErr
	}
	defer f.Close()

	host, port, err := net.SplitHostPort(host)
	if err != nil {
		port = fmt.Sprintf("%d", defaultPort)
	}

	entry := fmt.Sprintf("[%s]:%s", host, port)
	if ip := net.ParseIP(host); ip != nil {
		if ip.To4() != nil {
			if port == fmt.Sprintf("%d", defaultPort) {
				entry = host
			}
		}
	}

	out := fmt.Sprintf("%s %s\n", entry, SerializePublicKey(key))
	_, fileErr := f.WriteString(out)
	return fileErr
}

// SerializePublicKey converts an ssh.PublicKey to printable bas64 string
func SerializePublicKey(k ssh.PublicKey) string {
	return k.Type() + " " + base64.StdEncoding.EncodeToString(k.Marshal())
}
