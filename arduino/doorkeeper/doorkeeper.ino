// Modul mikrofonu s analogovým výstupem

// nastavení propojovacích pinů
#define NOISE_THRESHOLD_PIN 2
#define NOISE_LEVEL_PIN A0
#define BUTTON_PIN 3
#define LED_PIN 13
#define BUZZER_PIN 8
#define RX_PIN 11
#define TX_PIN 10

#define BUZZER_DURATION 2000
#define LOOP_DELAY_TIME 10
#define BUTTON_HOLD_TIME 1500

// připojení knihovny SoftwareSerial
#include <SoftwareSerial.h>
// inicializace Bluetooth modulu z knihovny SoftwareSerial
SoftwareSerial bluetooth(TX_PIN, RX_PIN);

volatile bool button_state;
volatile bool previous_button_state;

unsigned long buzzer_start_time;
unsigned long button_down_time;

bool is_noise_up;
long noise_interrupt_time;
int max_noise_value;
bool is_button_hold;

void setup() {
  // inicializace komunikace po sériové lince
  // rychlostí 9600 baud
  Serial.begin(9600);

  // zahájení komunikace s Bluetooth modulem
  // skrze Softwarovou sériovou linku rychlostí 9600 baud
  bluetooth.begin(9600);
  
  // nastavení LED diody jako výstupní a její vypnutí
  pinMode(LED_PIN, OUTPUT);
  digitalWrite(LED_PIN, LOW);

  // nastaveni RELAY
  pinMode(BUZZER_PIN, OUTPUT);
  digitalWrite(BUZZER_PIN, HIGH); // LOW == relay is connected, HIGH == relay is disconnected

  // nastaveni BELL a BUTTON
  pinMode(NOISE_THRESHOLD_PIN, INPUT);
  pinMode(BUTTON_PIN, INPUT_PULLUP);

  attachInterrupt(digitalPinToInterrupt(NOISE_THRESHOLD_PIN), noise_interrupt, CHANGE);
  attachInterrupt(digitalPinToInterrupt(BUTTON_PIN), button_interrupt, CHANGE);

  button_state = previous_button_state = !digitalRead(BUTTON_PIN); // Pull-up resistors invert the logic, so true == off, false == on

  buzzer_start_time = 0;
  noise_interrupt_time = 0;
  is_noise_up = false;
  max_noise_value = 0;
  button_down_time = 0;
  is_button_hold = false;
}

void loop() {

  if (digitalRead(BUZZER_PIN) == LOW && ((millis() - buzzer_start_time) >= (unsigned long)BUZZER_DURATION)) { // overflow should not matter if calculating in unsigned integer arithmetics
    digitalWrite(BUZZER_PIN, HIGH); // turn off buzzer

    Serial.println("buzzer off");
    bluetooth.println("buzzer-off");
  }
  
  int noise_level = analogRead(NOISE_LEVEL_PIN);
  if (is_noise_up) {
    max_noise_value = max(max_noise_value, noise_level);
  }

  if (button_state != previous_button_state) {
    on_button_changed();
    previous_button_state = button_state;
  }

  if (button_state && !is_button_hold) {
    unsigned long button_down_duration = millis() - button_down_time;
    if (button_down_duration >= BUTTON_HOLD_TIME) {
      is_button_hold = true;
      bluetooth.println("button-hold");
    }
  }

  byte BluetoothData;
  // kontrola Bluetooth komunikace, pokud je dostupná nová
  // zpráva, tak nám tato funkce vrátí počet jejích znaků
  if (bluetooth.available() > 0) {
    // načtení prvního znaku ve frontě do proměnné
    BluetoothData=bluetooth.read();
    // dekódování přijatého znaku
    switch (BluetoothData) {
      // každý case obsahuje dekódování jednoho znaku
      case '0':
        // v případě přijetí znaku nuly vypneme LED diodu
        // a vypíšeme hlášku zpět do Bluetooth zařízení
        digitalWrite(LED_PIN, LOW);
        bluetooth.println("LED off");
        break;
      case '1':
        // v případě přijetí jedničky zapneme LED diodu
        // a vypíšeme hlášku zpět do Bluetooth zařízení
        digitalWrite(LED_PIN, HIGH);
        bluetooth.println("LED on");
        break;
      case 't':
        // v případě přejetí znaku 'a' vypíšeme
        // čas od spuštění Arduina
        bluetooth.print("Uptime: ");
        bluetooth.print(millis());
        bluetooth.println(" ms");
        break;
      case 'x':
        buzzer_start_time = millis(); // remember buzzer start time
        digitalWrite(BUZZER_PIN, LOW); // turn on buzzer
        Serial.println("buzzer on");
        bluetooth.println("buzzer-on");
        break;
      case '\r':
        // přesun na začátek řádku - znak CR
        break;
      case '\n':
        // odřádkování - znak LF
        break;
      default:
        // v případě přijetí ostatních znaků
        // vytiskneme informaci o neznámé zprávě
        bluetooth.println("invalid command");
    }
  }

  delay(LOOP_DELAY_TIME);
}

void noise_interrupt() {
  if (digitalRead(NOISE_THRESHOLD_PIN)) { // noise start
    is_noise_up = true;
    noise_interrupt_time = millis();
    max_noise_value = analogRead(NOISE_LEVEL_PIN);
    Serial.println("noise up");
  }
  else { // noise stop
    if (is_noise_up) { // if it is too fast, do not react
      is_noise_up = false;
      long noise_interval = millis() - noise_interrupt_time;
    
      bluetooth.print("noise,");
      bluetooth.print(noise_interval);
      bluetooth.print(",");
      bluetooth.println(max_noise_value);
    }
    Serial.println("noise down");
  }
}

void button_interrupt() {
  button_state = !digitalRead(BUTTON_PIN);
  if (button_state) {
    button_down_time = millis();
  }
}

void on_button_changed() {
  if (button_state) {
    Serial.println("button down");
    bluetooth.println("button-down");
  }
  else {
    is_button_hold = false;
    Serial.println("button up");
    bluetooth.println("button-up");
  }
}
