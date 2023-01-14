package utils

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/base64"
	"encoding/pem"
	"fmt"
	"io/ioutil"
	"log"
	"os"
	"os/user"
	"path/filepath"
	"syscall"

	"github.com/ferama/rospo/pkg/cache"
	"golang.org/x/crypto/ssh"
	"golang.org/x/crypto/ssh/knownhosts"
	"golang.org/x/crypto/ssh/terminal"
)

// GeneratePrivateKey generate an rsa key (actually used from the sshd server)
func GeneratePrivateKey() (*rsa.PrivateKey, error) {
	bitSize := 4096
	privateKey, _ := rsa.GenerateKey(rand.Reader, bitSize)

	// Validate Private Key
	privateKey.Validate()
	// log.Println("private key generated")
	return privateKey, nil
}

// EncodePrivateKeyToPEM converts a private key object to PEM
func EncodePrivateKeyToPEM(privateKey *rsa.PrivateKey) []byte {
	privDER := x509.MarshalPKCS1PrivateKey(privateKey)

	// pem.Block
	privBlock := pem.Block{
		Type:    "RSA PRIVATE KEY",
		Headers: nil,
		Bytes:   privDER,
	}

	// Private key in PEM format
	privatePEM := pem.EncodeToMemory(&privBlock)
	return privatePEM
}

// GeneratePublicKey generates a public key from a private one
func GeneratePublicKey(key *rsa.PublicKey) ([]byte, error) {
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

	if err := ioutil.WriteFile(path, keyBytes, 0600); err != nil {
		log.Println(err)
		return err
	}
	return nil
}

func isKeyEncryptedWithPassphrase(keyPath string) (bool, error) {
	keyData, err := ioutil.ReadFile(keyPath)
	if err != nil {
		return false, err
	}

	_, err = ssh.ParsePrivateKey(keyData)
	if err != nil {
		return true, nil
	}

	return false, nil
}

// LoadIdentityFile reads a public key file and loads the keys to
// an ssh.PublicKeys object
func LoadIdentityFile(file string) (ssh.AuthMethod, error) {
	path, _ := ExpandUserHome(file)

	usr, _ := user.Current()
	// no path is set, try with a reasonable default
	if path == "" {
		path = filepath.Join(usr.HomeDir, ".ssh", "id_rsa")
	}

	isKeyEncrypted, err := isKeyEncryptedWithPassphrase(file)
	if err != nil {
		return nil, err
	}

	if isKeyEncrypted {
		password := []byte(cache.CachedKeyPw)

		if string(cache.CachedKeyPw) == "" {
			fmt.Println("Enter passphrase for SSH key")
			password, err = terminal.ReadPassword(int(syscall.Stdin))
			if err != nil {
				return nil, err
			}
		}

		for cache.CacheKeyPass != "y" && cache.CacheKeyPass != "n" {
			fmt.Println("Cache the key password? (y/n) (insecure)")
			cacheInput, err := terminal.ReadPassword(int(syscall.Stdin))
			if err != nil {
				return nil, err
			}

			if string(cacheInput) == "y" {
				cache.CacheKeyPass = "y"
				cache.CachedKeyPw = password
				break
			} else if string(cacheInput) == "n" {
				cache.CacheKeyPass = "n"
				cache.CachedKeyPw = []byte("")
				break
			}

			fmt.Println("invalid option (pick y/n)")
		}

		buffer, err := ioutil.ReadFile(path)
		if err != nil {
			return nil, fmt.Errorf("cannot read SSH identity key file %s", path)
		}

		key, err := ssh.ParsePrivateKeyWithPassphrase(buffer, password)
		if err != nil {
			return nil, fmt.Errorf("cannot parse SSH identity key file %s", file)
		}

		return ssh.PublicKeys(key), nil
	} else {
		buffer, err := ioutil.ReadFile(path)
		if err != nil {
			return nil, fmt.Errorf("cannot read SSH identity key file %s", path)
		}

		key, err := ssh.ParsePrivateKey(buffer)
		if err != nil {
			return nil, fmt.Errorf("cannot parse SSH identity key file %s", file)
		}
		return ssh.PublicKeys(key), nil
	}
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

	knownHosts := knownhosts.Normalize(host)
	out := fmt.Sprintf("%s\n", knownhosts.Line([]string{knownHosts}, key))
	_, fileErr := f.WriteString(out)
	return fileErr
}

// SerializePublicKey converts an ssh.PublicKey to printable bas64 string
func SerializePublicKey(k ssh.PublicKey) string {
	return k.Type() + " " + base64.StdEncoding.EncodeToString(k.Marshal())
}
