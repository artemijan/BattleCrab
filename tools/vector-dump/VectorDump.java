import java.math.BigInteger;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.MessageDigest;
import java.security.interfaces.RSAPrivateKey;
import java.security.interfaces.RSAPublicKey;
import java.security.spec.RSAKeyGenParameterSpec;
import java.util.Base64;

import javax.crypto.Cipher;

import org.l2jmobius.commons.network.Buffer;
import org.l2jmobius.loginserver.crypt.NewCrypt;

/**
 * Dumps golden vectors from the real Java crypto classes as JSON, for the Rust
 * port's parity tests. Run once; output is committed to the Rust repo.
 */
public class VectorDump
{
	/** Little-endian byte[] Buffer, mirroring the LE ByteBuffers of Async-mmocore. */
	static class ByteArrayBuffer implements Buffer
	{
		private final byte[] data;
		private int limit;

		ByteArrayBuffer(byte[] data)
		{
			this.data = data;
			this.limit = data.length;
		}

		public byte readByte(int index)
		{
			return data[index];
		}

		public void writeByte(int index, byte value)
		{
			data[index] = value;
		}

		public short readShort(int index)
		{
			return (short) ((data[index] & 0xff) | ((data[index + 1] & 0xff) << 8));
		}

		public void writeShort(int index, short value)
		{
			data[index] = (byte) value;
			data[index + 1] = (byte) (value >> 8);
		}

		public int readInt(int index)
		{
			return (data[index] & 0xff) | ((data[index + 1] & 0xff) << 8) | ((data[index + 2] & 0xff) << 16) | ((data[index + 3] & 0xff) << 24);
		}

		public void writeInt(int index, int value)
		{
			data[index] = (byte) value;
			data[index + 1] = (byte) (value >> 8);
			data[index + 2] = (byte) (value >> 16);
			data[index + 3] = (byte) (value >> 24);
		}

		public int limit()
		{
			return limit;
		}

		public void limit(int newLimit)
		{
			limit = newLimit;
		}
	}

	static String hex(byte[] b)
	{
		final StringBuilder sb = new StringBuilder();
		for (byte x : b)
		{
			sb.append(String.format("%02x", x));
		}
		return sb.toString();
	}

	static byte[] deterministicBytes(int len, int seed)
	{
		final byte[] out = new byte[len];
		int state = seed;
		for (int i = 0; i < len; i++)
		{
			state = (state * 1103515245) + 12345;
			out[i] = (byte) (state >> 16);
		}
		return out;
	}

	public static void main(String[] args) throws Exception
	{
		final StringBuilder json = new StringBuilder();
		json.append("{\n");

		// --- Blowfish (L2 LE variant) with the static GS-link key and a runtime-style key.
		final byte[] staticKey = "_;5.]94-31==-%xT!^[$\000".getBytes();
		final byte[] runtimeKey = deterministicBytes(16, 42);
		final byte[] plain = deterministicBytes(32, 7);

		byte[] work = plain.clone();
		new NewCrypt(staticKey).crypt(new ByteArrayBuffer(work), 0, work.length);
		json.append("  \"blowfish_static_key\": \"").append(hex(staticKey)).append("\",\n");
		json.append("  \"blowfish_runtime_key\": \"").append(hex(runtimeKey)).append("\",\n");
		json.append("  \"blowfish_plain\": \"").append(hex(plain)).append("\",\n");
		json.append("  \"blowfish_static_encrypted\": \"").append(hex(work)).append("\",\n");

		work = plain.clone();
		new NewCrypt(runtimeKey).crypt(new ByteArrayBuffer(work), 0, work.length);
		json.append("  \"blowfish_runtime_encrypted\": \"").append(hex(work)).append("\",\n");

		// --- Checksum.
		final byte[] checksumData = deterministicBytes(24, 99); // last 4 bytes get overwritten
		work = checksumData.clone();
		NewCrypt.appendChecksum(new ByteArrayBuffer(work), 0, work.length);
		json.append("  \"checksum_input\": \"").append(hex(checksumData)).append("\",\n");
		json.append("  \"checksum_output\": \"").append(hex(work)).append("\",\n");

		// --- XOR pass (Init packet outer layer).
		final byte[] xorData = deterministicBytes(40, 123);
		final int xorKey = 0x1234abcd;
		work = xorData.clone();
		NewCrypt.encXORPass(new ByteArrayBuffer(work), 0, work.length, xorKey);
		json.append("  \"xor_input\": \"").append(hex(xorData)).append("\",\n");
		json.append("  \"xor_key\": ").append(xorKey).append(",\n");
		json.append("  \"xor_output\": \"").append(hex(work)).append("\",\n");

		// --- RSA: modulus scramble + raw nopadding block.
		final KeyPairGenerator keygen = KeyPairGenerator.getInstance("RSA");
		keygen.initialize(new RSAKeyGenParameterSpec(1024, RSAKeyGenParameterSpec.F4));
		final KeyPair pair = keygen.generateKeyPair();
		final RSAPublicKey pub = (RSAPublicKey) pair.getPublic();
		final RSAPrivateKey prv = (RSAPrivateKey) pair.getPrivate();

		// scrambleModulus is private in ScrambledKeyPair; replicated here verbatim.
		byte[] scrambled = pub.getModulus().toByteArray();
		if ((scrambled.length == 0x81) && (scrambled[0] == 0x00))
		{
			final byte[] temp = new byte[0x80];
			System.arraycopy(scrambled, 1, temp, 0, 0x80);
			scrambled = temp;
		}
		for (int i = 0; i < 4; i++)
		{
			final byte temp = scrambled[i];
			scrambled[i] = scrambled[0x4d + i];
			scrambled[0x4d + i] = temp;
		}
		for (int i = 0; i < 0x40; i++)
		{
			scrambled[i] = (byte) (scrambled[i] ^ scrambled[0x40 + i]);
		}
		for (int i = 0; i < 4; i++)
		{
			scrambled[0x0d + i] = (byte) (scrambled[0x0d + i] ^ scrambled[0x34 + i]);
		}
		for (int i = 0; i < 0x40; i++)
		{
			scrambled[0x40 + i] = (byte) (scrambled[0x40 + i] ^ scrambled[i]);
		}

		json.append("  \"rsa_modulus\": \"").append(pub.getModulus().toString(16)).append("\",\n");
		json.append("  \"rsa_d\": \"").append(prv.getPrivateExponent().toString(16)).append("\",\n");
		json.append("  \"rsa_scrambled_modulus\": \"").append(hex(scrambled)).append("\",\n");

		// Raw block encrypted with the public key (as the client does); Rust must decrypt it.
		final byte[] rsaPlain = new byte[128];
		final byte[] credentials = deterministicBytes(30, 555);
		System.arraycopy(credentials, 0, rsaPlain, 0x5e, 30);
		rsaPlain[0] = 0; // keep block < modulus
		final Cipher rsaCipher = Cipher.getInstance("RSA/ECB/nopadding");
		rsaCipher.init(Cipher.ENCRYPT_MODE, pub);
		final byte[] rsaEncrypted = rsaCipher.doFinal(rsaPlain);
		json.append("  \"rsa_plain_block\": \"").append(hex(rsaPlain)).append("\",\n");
		json.append("  \"rsa_encrypted_block\": \"").append(hex(rsaEncrypted)).append("\",\n");

		// --- Password hash.
		final MessageDigest md = MessageDigest.getInstance("SHA");
		json.append("  \"password_plain\": \"L2jmobius123\",\n");
		json.append("  \"password_hash\": \"").append(Base64.getEncoder().encodeToString(md.digest("L2jmobius123".getBytes("UTF-8")))).append("\"\n");

		json.append("}\n");
		System.out.print(json);
	}
}
